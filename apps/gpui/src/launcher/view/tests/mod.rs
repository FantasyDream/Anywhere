mod acceptance_regressions;
mod activation_prefill_regressions;
mod async_regressions;
mod full_review;
mod lifecycle_regressions;
mod optimization_regressions;
mod tool_windows;

use super::*;
use crate::host::WebDocument;
use crate::launcher::view::navigation::LauncherStreamRecovery;
use anywhere_protocol::{FileObjectKind, HostView};
use gpui_kit::{
    test::TestWindowExt as _, AnyWindowHandle, TestAppContext, VisualTestContext, WindowOptions,
};

fn open_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<LauncherView>) {
    cx.update(gpui_kit::init);
    cx.update(crate::launcher::bind_keys);
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| LauncherView::new(window, cx)),
            )
        })
        .unwrap();
    (window, view)
}

fn primary_shortcut(key: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    format!("{modifier}-{key}")
}

fn dispatch_shortcut(cx: &mut TestAppContext, window: AnyWindowHandle, shortcut: &'static str) {
    let platform_shortcut = shortcut
        .strip_prefix("cmd-")
        .map_or_else(|| shortcut.to_owned(), primary_shortcut);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let keystroke =
            gpui_kit::Keystroke::parse_with_os(&platform_shortcut, cx.platform_info().os)
                .expect("valid shortcut");
        window.dispatch_keystroke(keystroke, cx);
    })
    .unwrap();
}

fn sync_test_viewport(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    size: gpui_kit::Size<gpui_kit::Pixels>,
) {
    cx.simulate_window_resize(window, size);
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert_eq!(window.viewport_size(), size);
        window.render_frame(cx);
    })
    .unwrap();
}

fn sync_test_viewport_to_desired(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    view: &Entity<LauncherView>,
) {
    let size = cx.update(|cx| view.read(cx).desired_window_size());
    sync_test_viewport(cx, window, size);
}

fn render_and_sync_test_viewport(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    view: &Entity<LauncherView>,
) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    let desired = cx.update(|cx| view.read(cx).desired_window_size());
    let bounds = cx
        .update_window(window, |_, window, _| window.bounds().size)
        .unwrap();
    assert_eq!(
        bounds, desired,
        "the deferred native resize must follow the rendered state"
    );
    sync_test_viewport(cx, window, desired);
}

fn scoped_state(scopes: Vec<anywhere_protocol::ScopeEntry>) -> anywhere_protocol::LauncherState {
    anywhere_protocol::LauncherState {
        scopes,
        ..Default::default()
    }
}

fn current_app_scope() -> anywhere_protocol::ScopeEntry {
    anywhere_protocol::ScopeEntry {
        id: "app.example".into(),
        label: "Editor".into(),
        is_current_app: true,
    }
}

fn file_row(id: &str, title: &str) -> LauncherRow {
    LauncherRow {
        id: SharedString::from(id.to_owned()),
        title: SharedString::from(title.to_owned()),
        subtitle: Some(SharedString::from(format!("/fixtures/{title}"))),
        shortcut: None,
        icon: "file".to_owned(),
        kind: CandidateKind::File,
        enabled: true,
        is_menu: false,
        menu_path: Vec::new(),
        command_id: None,
        file_kind: Some(FileObjectKind::File),
        source_label: Some(SharedString::from("Files")),
        icon_identity: None,
        execution_context_id: None,
        primary_action: false,
    }
}

#[gpui_kit::test]
fn focusing_an_open_workspace_keeps_the_live_snapshot_stream(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        let expected_generation = view.update(cx, |view, cx| {
            view.presence.presented = true;
            view.screen = Screen::Launcher;
            view.input_mode = InputMode::Environment;
            view.live_state.generation = 41;
            assert!(view.activate_from_entry(InputMode::Environment, false, window, cx));
            view.live_state.generation
        });
        assert_eq!(expected_generation, 41);
    })
    .unwrap();
}

#[gpui_kit::test]
fn current_app_selects_a_late_scope_once_without_overriding_user_navigation(
    cx: &mut TestAppContext,
) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Environment;
            view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
            view.apply_state(scoped_state(Vec::new()), cx);
            assert_eq!(view.model.selected_scope, None);
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::PendingCurrentApp
            );

            view.apply_state(scoped_state(vec![current_app_scope()]), cx);
            assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::UserControlled
            );

            view.model.select_scope(None);
            view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
            view.mark_scope_user_controlled();
            view.apply_state(scoped_state(vec![current_app_scope()]), cx);
            assert_eq!(view.model.selected_scope, None);
        });
    });
}

#[gpui_kit::test]
fn switching_back_to_current_app_uses_an_existing_scope_with_a_query(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Search;
            view.model.scopes = vec![
                crate::launcher::LauncherScope {
                    id: "app.example".into(),
                    label: "Editor".into(),
                    is_current_app: true,
                },
                crate::launcher::LauncherScope {
                    id: "files".into(),
                    label: "Files".into(),
                    is_current_app: false,
                },
            ];
            view.model.set_query("open".into());
            view.model.select_scope(None);
            view.select_root_mode(InputMode::Environment, cx);
            assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::UserControlled
            );
        });
    });
}

#[gpui_kit::test]
fn switching_back_to_current_app_waits_for_a_late_scope_with_a_query(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Search;
            view.model.scopes.clear();
            view.model.set_query("open".into());
            view.model.select_scope(None);
            view.select_root_mode(InputMode::Environment, cx);
            assert_eq!(view.model.selected_scope, None);
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::PendingCurrentAppNavigation
            );
            assert!(view.model.loading);
            assert!(view.model.rows.is_empty());

            let mut state = scoped_state(vec![current_app_scope()]);
            state.input = "open".into();
            view.apply_state(state, cx);
            assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::UserControlled
            );
        });
    });
}

#[gpui_kit::test]
fn current_app_navigation_finishes_when_no_app_scope_exists(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Search;
            view.model.scopes.clear();
            view.model.set_query("open".into());
            view.select_root_mode(InputMode::Environment, cx);
            assert!(view.model.loading);

            let mut state = scoped_state(Vec::new());
            state.input = "open".into();
            state.context_complete = true;
            view.apply_state(state, cx);
            assert!(!view.model.loading);
            assert!(view.model.rows.is_empty());
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::UserControlled
            );
        });
    });
}

#[gpui_kit::test]
fn entry_default_current_app_is_still_cancelled_by_a_query(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Environment;
            view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
            view.model.set_query("open".into());
            let mut state = scoped_state(vec![current_app_scope()]);
            state.input = "open".into();
            view.apply_state(state, cx);
            assert_eq!(view.model.selected_scope, None);
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::PendingCurrentApp
            );
        });
    });
}

#[gpui_kit::test]
fn programmatic_editor_reset_does_not_cancel_a_pending_current_scope(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input
                .update(cx, |input, cx| input.insert("old", window, cx));
            view.input_mode = InputMode::Environment;
            view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
            view.sync_launcher_editor(String::new(), cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            assert_eq!(
                view.scope_selection_intent,
                ScopeSelectionIntent::PendingCurrentApp
            );
            view.apply_state(scoped_state(vec![current_app_scope()]), cx);
            assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
        });
    });
}

#[gpui_kit::test]
fn tab_cycles_only_visible_scopes_from_the_focused_editor(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.model.scopes = vec![
                crate::launcher::LauncherScope {
                    id: "app.example".into(),
                    label: "Editor".into(),
                    is_current_app: true,
                },
                crate::launcher::LauncherScope {
                    id: "files".into(),
                    label: "Files".into(),
                    is_current_app: false,
                },
            ];
            view.model.select_scope(None);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("launcher-search-input", cx);
        assert!(
            view.update(cx, |view, cx| view.active_editor_focused(window, cx)),
            "the launcher editor must own focus before Tab navigation"
        );
        assert!(view.read(cx).has_scope_bar());
        window.press("tab", cx);
        assert_eq!(
            view.read(cx).model.selected_scope.as_deref(),
            Some("app.example")
        );
        window.press("tab", cx);
        assert_eq!(view.read(cx).model.selected_scope.as_deref(), Some("files"));
        window.press("tab", cx);
        assert_eq!(
            view.read(cx).model.selected_scope.as_deref(),
            Some("app.example")
        );
        window.press("shift-tab", cx);
        assert_eq!(view.read(cx).model.selected_scope.as_deref(), Some("files"));
    })
    .unwrap();
}

#[test]
fn scope_tab_shortcuts_reject_command_and_option_modifiers() {
    assert!(is_scope_tab_shortcut("tab", KeyModifiers::default()));
    assert!(is_scope_tab_shortcut(
        "tab",
        KeyModifiers {
            control: true,
            shift: true,
            modified: true,
            ..Default::default()
        }
    ));
    assert!(!is_scope_tab_shortcut(
        "tab",
        KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        }
    ));
    assert!(!is_scope_tab_shortcut(
        "tab",
        KeyModifiers {
            alt: true,
            modified: true,
            ..Default::default()
        }
    ));
}

#[test]
fn identical_launcher_sizes_do_not_trigger_native_resizes() {
    let same = gpui_kit::size(px(648.), px(72.));
    assert!(!window_size_changed(same, same));
    assert!(!window_size_changed(
        same,
        gpui_kit::size(px(648.4), px(72.4))
    ));
    assert!(window_size_changed(
        same,
        gpui_kit::size(px(648.), px(128.))
    ));
}

#[test]
fn effective_window_size_matches_the_native_visible_frame_budget() {
    let natural = gpui_kit::size(px(648.), px(600.));
    let available = gpui_kit::size(px(800.), px(600.));
    let effective = effective_window_size(natural, available);
    assert_eq!(effective, gpui_kit::size(px(648.), px(568.)));
    assert_eq!(effective_window_size(effective, available), effective);
    assert!(!window_size_changed(effective, effective));

    let narrow = gpui_kit::size(px(420.), px(380.));
    assert_eq!(
        effective_window_size(natural, narrow),
        gpui_kit::size(px(388.), px(348.))
    );
}

#[test]
fn clipboard_confirmation_rejects_stale_or_loading_submissions() {
    let clear = ClipboardConfirmation::Clear;
    assert!(clipboard_confirmation_is_submittable(
        false,
        Some(clear),
        clear
    ));
    assert!(!clipboard_confirmation_is_submittable(
        true,
        Some(clear),
        clear
    ));
    assert!(!clipboard_confirmation_is_submittable(false, None, clear));
    assert!(!clipboard_confirmation_is_submittable(
        false,
        Some(ClipboardConfirmation::SetRetention(30)),
        ClipboardConfirmation::SetRetention(7),
    ));
}

#[test]
fn tool_pages_consume_unmodified_tab_navigation() {
    for screen in [
        Screen::Clipboard,
        Screen::ClipboardShelf,
        Screen::ApplicationHistory,
        Screen::UnsupportedTool,
    ] {
        assert!(is_tool_page_tab_shortcut(
            screen,
            "tab",
            KeyModifiers::default()
        ));
        assert!(is_tool_page_tab_shortcut(
            screen,
            "tab",
            KeyModifiers {
                shift: true,
                modified: true,
                ..Default::default()
            }
        ));
        assert!(!is_tool_page_tab_shortcut(
            screen,
            "tab",
            KeyModifiers {
                control: true,
                modified: true,
                ..Default::default()
            }
        ));
    }
    assert!(!is_tool_page_tab_shortcut(
        Screen::Tasks,
        "tab",
        KeyModifiers::default()
    ));
    assert!(!is_tool_page_tab_shortcut(
        Screen::Settings,
        "tab",
        KeyModifiers::default()
    ));
}

#[gpui_kit::test]
fn tasks_search_shortcut_only_focuses_the_visible_filter(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = |id: &str| crate::host::ExecutionRow {
        id: id.to_owned(),
        title: id.to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.surface_focus_handle.focus(window, cx);
            assert!(view.tasks.rows.is_empty());
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));

            view.apply_tasks_rows(vec![row("task")], cx);
            view.tasks_filter_input
                .update(cx, |input, cx| input.set_value("task", window, cx));
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(view
            .tasks_filter_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
        assert_eq!(view.tasks_filter_input.read(cx).selected_range(), 0..4);
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_command_k_opens_the_current_task_actions(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let command = KeyModifiers {
        command: true,
        modified: true,
        ..Default::default()
    };
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "npm Run · task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "npm run task".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
        output_revision: 1,
        exit_code: Some(0),
        started_at_ms: 0,
        finished_at_ms: Some(1),
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.surface_focus_handle.focus(window, cx);
            view.apply_tasks_rows(vec![row.clone()], cx);
            view.tasks.detail = Some(crate::host::ExecutionDetail {
                row,
                stdout: "ready".into(),
                stderr: "".into(),
            });
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.tasks.task_actions_open);
            assert!(!view.tasks.tool_options_open);
            assert_eq!(view.tasks.task_action_selected, 0);

            view.handle_key_with_repeat("k", command, true, window, cx);
            assert!(
                view.tasks.task_actions_open,
                "held command-k must not toggle task Actions closed"
            );
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.tasks.task_action_selected, 1);
            view.handle_key("up", KeyModifiers::default(), window, cx);
            assert_eq!(view.tasks.task_action_selected, 0);

            view.handle_key("enter", KeyModifiers::default(), window, cx);
            assert!(!view.tasks.task_actions_open);
            assert_eq!(view.notice, Some(l10n::command_copied(Locale::En)));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_command_k_uses_window_options_until_a_detail_is_visible(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let command = KeyModifiers {
        command: true,
        modified: true,
        ..Default::default()
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.surface_focus_handle.focus(window, cx);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.tasks.tool_options_open);
            assert!(!view.tasks.task_actions_open);
            view.handle_key_with_repeat("k", command, true, window, cx);
            assert!(
                view.tasks.tool_options_open,
                "held command-k must not toggle Tool options closed"
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_tool_options_expose_the_launcher(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.tasks.tool_options_open = true;
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, _| assert!(window.try_find("tasks-open-launcher").is_some()));
}

#[gpui_kit::test]
fn tasks_tool_options_keyboard_selects_launcher_and_closes_with_escape(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};

    let (window, view) = open_view(cx);
    let restored = Rc::new(Cell::new(false));
    let observed = restored.clone();
    let _event = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::RestoreLauncher {
                observed.set(true);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.surface_focus_handle.focus(window, cx);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.tasks.tool_options_open);
            assert_eq!(view.tasks.tool_option_selected, 0);

            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.tasks.tool_option_selected, 1);
            view.handle_key("enter", KeyModifiers::default(), window, cx);
            assert!(!view.tasks.tool_options_open);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.tasks.tool_options_open);
            assert_eq!(view.screen, Screen::Tasks);
        });
    })
    .unwrap();
    assert!(restored.get());
}

#[gpui_kit::test]
fn repeated_tasks_enter_does_not_repeat_the_tool_option(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};

    let (window, view) = open_view(cx);
    let restored = Rc::new(Cell::new(false));
    let observed = restored.clone();
    let _event = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::RestoreLauncher {
                observed.set(true);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.tasks.tool_options_open = true;
            view.tasks.tool_option_selected = 1;

            view.handle_tasks_navigation_key("enter", true, window, cx);
            assert!(view.tasks.tool_options_open);
            assert!(!restored.get());

            view.handle_tasks_navigation_key("enter", false, window, cx);
            assert!(!view.tasks.tool_options_open);
        });
    })
    .unwrap();
    assert!(restored.get());
}

#[gpui_kit::test]
fn clipboard_shelf_search_shortcut_selects_the_existing_query(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.surface_focus_handle.focus(window, cx);
            view.clipboard_input
                .update(cx, |input, cx| input.set_value("history", window, cx));
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(view
            .clipboard_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
        assert_eq!(view.clipboard_input.read(cx).selected_range(), 0..7);
    })
    .unwrap();
}

#[gpui_kit::test]
fn selecting_a_clipboard_row_restores_search_focus(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.blur(cx);
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.select_clipboard_row(0, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        assert!(view
            .read(cx)
            .clipboard_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
    })
    .unwrap();
}

#[gpui_kit::test]
fn typing_in_clipboard_search_closes_actions(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.clipboard.actions_open = true;
            view.clipboard.action_target = Some(1);
            view.clipboard.action_selected = 3;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("clipboard-shelf-search", cx);
        window.input("history", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        view.read_with(cx, |view, _| {
            assert!(!view.clipboard.actions_open);
            assert_eq!(view.clipboard.action_target, None);
            assert_eq!(view.clipboard.action_selected, 0);
            assert_eq!(view.clipboard.query, "history");
        });
    });
}

#[gpui_kit::test]
fn clipboard_actions_close_before_handling_command_shortcuts(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.surface_focus_handle.focus(window, cx);
            view.clipboard.actions_open = true;
            view.clipboard.action_target = Some(1);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, _window, cx| {
        view.update(cx, |view, _| {
            assert!(!view.clipboard.actions_open);

            view.clipboard.actions_open = true;
            view.clipboard.action_target = Some(1);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(!view.clipboard.actions_open);
        assert!(view
            .clipboard_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
    })
    .unwrap();
}

#[gpui_kit::test]
fn clipboard_refresh_survives_a_stale_action_target(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(crate::launcher::bind_keys);
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
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                },
            )
        })
        .unwrap();

    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, cx| {
            view.clipboard.actions_open = true;
            view.clipboard.action_target = Some(999);
            view.clipboard.action_selected =
                LauncherView::clipboard_refresh_action(Screen::ClipboardShelf);
            let generation = view.clipboard.query_generation;

            view.execute_clipboard_action(view.clipboard.action_selected, cx);

            assert_eq!(view.clipboard.query_generation, generation + 1);
            assert!(!view.clipboard.actions_open);
            assert_eq!(view.clipboard.action_target, None);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn held_return_copies_a_shelf_item_once(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;

    cx.update(gpui_kit::init);
    cx.update(crate::launcher::bind_keys);
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
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                },
            )
        })
        .unwrap();

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, _| {
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 4)),
                notice: None,
                items: vec![crate::host::ClipboardItem {
                    id: 1,
                    summary: "fixture".into(),
                    kind: "text".into(),
                    copied_at_ms: 0,
                    byte_count: 4,
                }],
            });
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
        });
        let generation = view.read(cx).clipboard.operation_generation;

        window.key_down("enter", true, cx);
        assert_eq!(view.read(cx).clipboard.operation_generation, generation);

        window.key_down("enter", false, cx);
        assert_eq!(view.read(cx).clipboard.operation_generation, generation + 1);

        window.key_up("enter", cx);
        window.key_down("enter", true, cx);
        assert_eq!(view.read(cx).clipboard.operation_generation, generation + 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn held_command_k_does_not_toggle_shelf_actions_twice(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;

    cx.update(gpui_kit::init);
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
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                },
            )
        })
        .unwrap();

    cx.update_window(window, |_, window, cx| {
        window.key_down(&primary_shortcut("k"), false, cx);
        assert!(view.read(cx).clipboard.actions_open);

        crate::platform::set_key_event_is_repeat_for_test(true);
        window.key_down(&primary_shortcut("k"), true, cx);
        crate::platform::set_key_event_is_repeat_for_test(false);
        assert!(view.read(cx).clipboard.actions_open);
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_window_does_not_keep_a_hidden_launcher_editor_focused(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(crate::launcher::bind_keys);
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
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| LauncherView::tasks_window(window, cx, live, Locale::En))
                },
            )
        })
        .unwrap();

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(!view.input.read(cx).focus_handle(cx).is_focused(window));
            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn removing_every_task_blurs_the_hidden_filter(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.surface_focus_handle.focus(window, cx);
            view.apply_tasks_rows(vec![row], cx);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
            view.apply_tasks_rows(Vec::new(), cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_escape_clears_the_filter_and_returns_focus_to_the_list(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(vec![row], cx);
            view.tasks_filter_input
                .update(cx, |input, cx| input.set_value("task", window, cx));
            view.tasks.set_filter("task".to_owned());
            view.tasks_filter_input
                .update(cx, |input, cx| input.focus(window, cx));
            assert!(view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));

            view.handle_key("escape", KeyModifiers::default(), window, cx);

            assert!(view.tasks.filter.is_empty());
            assert_eq!(view.tasks_filter_input.read(cx).value(), "");
            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_enter_from_the_filter_moves_focus_to_the_list(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(vec![row], cx);
            view.tasks_filter_input
                .update(cx, |input, cx| input.focus(window, cx));

            view.handle_key("enter", KeyModifiers::default(), window, cx);

            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn clicking_a_task_row_moves_focus_out_of_the_filter(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(vec![row], cx);
            view.tasks_filter_input
                .update(cx, |input, cx| input.focus(window, cx));
            view.select_task_row(0, window, cx);
        });
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(!view
                .tasks_filter_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn starting_runtime_wakes_only_the_shell_without_reserving_a_session(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(crate::launcher::bind_keys);
    let (window, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| LauncherView::starting(window, cx, Locale::En)),
            )
        })
        .unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.activate_from_entry(InputMode::Environment, true, window, cx));
            assert!(view.presence.presented);
            assert_eq!(view.runtime_state, RuntimeState::Starting);
            assert_eq!(
                view.notice.as_deref(),
                Some(l10n::starting_anywhere(Locale::En).as_str())
            );
            assert_eq!(view.live_state.session_id, None);
            assert!(view.live_state.snapshot_gate.is_none());
            assert!(!view.open_settings(window, cx));
            assert_eq!(view.screen, Screen::Launcher);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn preview_settings_open_without_a_live_runtime(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert_eq!(view.runtime_state, RuntimeState::Preview);
            assert!(view.live.is_none());
            assert!(view.open_settings(window, cx));
            assert_eq!(view.screen, Screen::Settings);
        });
        window.render_frame(cx);
        assert!(window.try_find("settings-surface").is_some());
    })
    .unwrap();
}

#[test]
fn file_and_action_icons_preserve_the_flutter_vocabulary() {
    assert_eq!(
        file_icon_appearance("Projects", Some(FileObjectKind::Directory)).glyph,
        IconName::Folder
    );
    assert_eq!(
        file_icon_appearance("report.pdf", Some(FileObjectKind::File)).glyph,
        IconName::FileText
    );
    assert_eq!(
        file_icon_appearance("report.pdf", Some(FileObjectKind::File)).stamp,
        Some("PDF")
    );
    assert_eq!(
        file_icon_appearance("notes.md", Some(FileObjectKind::File)).glyph,
        IconName::Type
    );
    assert_eq!(
        file_icon_appearance("budget.xlsx", Some(FileObjectKind::File)).glyph,
        IconName::FileSpreadsheet
    );
    assert_eq!(
        file_icon_appearance("cover.png", Some(FileObjectKind::File)).glyph,
        IconName::Image
    );
    assert_eq!(
        file_icon_appearance("archive.zip", Some(FileObjectKind::File)).glyph,
        IconName::FileArchive
    );
    assert_eq!(
        file_icon_appearance("main.rs", Some(FileObjectKind::File)).glyph,
        IconName::FileCode
    );
    assert_eq!(
        file_icon_appearance("config.json", Some(FileObjectKind::File)).glyph,
        IconName::FileCog
    );
    assert_eq!(
        action_icon_appearance("folder-open", CandidateKind::Action).glyph,
        IconName::FolderOpen
    );
    assert_eq!(
        action_icon_appearance("copy-url", CandidateKind::Action).glyph,
        IconName::Link
    );
    assert_eq!(
        action_icon_appearance("git-diff", CandidateKind::Action).glyph,
        IconName::Diff
    );
    assert_eq!(
        action_icon_appearance("developer-test", CandidateKind::Action).glyph,
        IconName::FlaskConical
    );
}

#[test]
fn permission_failures_open_the_matching_privacy_pane() {
    assert_eq!(
        privacy_pane_for_capture(
            ContextSource::FinderSelection,
            ContextCaptureStatus::PermissionDenied,
        ),
        Some(PrivacySettingsPane::Automation)
    );
    assert_eq!(
        privacy_pane_for_capture(
            ContextSource::BrowserCurrentPage,
            ContextCaptureStatus::PermissionDenied,
        ),
        Some(PrivacySettingsPane::Automation)
    );
    assert_eq!(
        privacy_pane_for_capture(
            ContextSource::SelectedText,
            ContextCaptureStatus::PermissionDenied,
        ),
        Some(PrivacySettingsPane::Accessibility)
    );
    assert_eq!(
        privacy_pane_for_capture(
            ContextSource::Clipboard,
            ContextCaptureStatus::PermissionDenied,
        ),
        Some(PrivacySettingsPane::Clipboard)
    );
    assert_eq!(
        privacy_pane_for_capture(ContextSource::SelectedText, ContextCaptureStatus::TimedOut,),
        None
    );
}

#[gpui_kit::test]
fn settings_expose_privacy_recovery_actions(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.apply_state(
                anywhere_protocol::LauncherState {
                    context_captures: vec![anywhere_protocol::ContextCapturePresentation {
                        text: None,
                        clipboard_age_ms: None,
                        source: ContextSource::FinderSelection,
                        status: ContextCaptureStatus::PermissionDenied,
                        captured_at_ms: 0,
                    }],
                    ..Default::default()
                },
                cx,
            );
            view.settings_editor.model.selected = SettingsCategory::Context;
            view.screen = Screen::Settings;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .try_find("context-open-settings-FinderSelection")
                .is_some(),
            !cfg!(windows)
        );

        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::Search;
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.try_find("open-full-disk-access").is_some(),
            !cfg!(windows)
        );
        assert!(window.try_find("authorize-search-directories").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn file_search_uses_the_original_fixed_page_shape(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.query = "report".to_owned();
            view.model.rows = vec![file_row("report", "report.pdf")].into();
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("file-search-back").is_some());
        assert!(window.try_find("file-search-controls").is_some());
        assert!(window.try_find("file-search-input").is_some());
        assert!(window.try_find("launcher-mode-label").is_some());
        view.update(cx, |view, _| {
            assert_eq!(view.file_range_label(), "File Search");
            assert_eq!(view.desired_window_size().height, px(600.));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn standalone_tasks_and_settings_keep_fixed_headers_separate(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    let tasks_header = visual
        .debug_bounds("tasks-toolbar")
        .expect("tasks toolbar bounds");
    assert!(
        tasks_header.origin.y >= px(metrics::TITLE_BAR_INSET),
        "the Tasks header must stay below shared window controls"
    );

    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.standalone_tasks = false;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            cx.notify();
        });
        window.render_frame(cx);
    });
    let title = visual
        .debug_bounds("settings-toolbar-title")
        .expect("settings title bounds");
    let actions = visual
        .debug_bounds("settings-toolbar-actions")
        .expect("settings action bounds");
    assert!(
        title.origin.x + title.size.width <= actions.origin.x,
        "the settings title must not overlap Save / Done"
    );
    let dirty = visual
        .debug_bounds("settings-dirty-label")
        .expect("settings dirty label bounds");
    assert!(
        dirty.size.width > px(120.),
        "the dirty status must render its full label instead of the old hard truncation"
    );
    assert!(
        dirty.origin.x + dirty.size.width <= actions.right(),
        "the dirty status must stay inside the toolbar actions"
    );
}

#[gpui_kit::test]
fn content_mode_uses_the_extra_header_height_and_two_line_editor(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Content;
            view.model.rows.clear();
            assert_eq!(view.desired_window_size().height, px(184.5));
            assert!(view.content_input.read(cx).is_multi_line());
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-content-source-icon").is_some());
        assert!(window.try_find("launcher-search-input-slot").is_some());
        assert!(window.try_find("launcher-result-heading").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn core_snapshot_inputs_are_reflected_in_the_active_editor(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.apply_state(
                anywhere_protocol::LauncherState {
                    input: "report".to_owned(),
                    revision: 1,
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(view.input.read(cx).value(), "report");

            view.input_mode = InputMode::Content;
            view.apply_state(
                anywhere_protocol::LauncherState {
                    input: "captured text".to_owned(),
                    revision: 2,
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(view.content_input.read(cx).value(), "captured text");
        });
    });
}

#[gpui_kit::test]
fn result_heading_tracks_actions_and_menu_breadcrumbs(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.model.set_query("git".to_owned());
            assert_eq!(
                view.result_heading(),
                None,
                "ordinary search stays heading-free like the Flutter UI"
            );

            view.model.show_object_actions();
            assert_eq!(view.result_heading(), Some(("Actions".to_owned(), None)));
            view.model.hide_object_actions();

            view.model.rows = vec![LauncherRow {
                id: SharedString::from("menu-item"),
                title: SharedString::from("Menu item"),
                subtitle: None,
                shortcut: None,
                icon: "menu".to_owned(),
                kind: CandidateKind::Action,
                enabled: true,
                is_menu: true,
                menu_path: vec!["View".to_owned(), "Layout".to_owned()],
                command_id: None,
                file_kind: None,
                source_label: None,
                icon_identity: None,
                execution_context_id: None,
                primary_action: false,
            }]
            .into();
            assert!(view.model.open_selected_menu());
            assert_eq!(
                view.result_heading(),
                Some(("Menus › View › Layout".to_owned(), None))
            );
        });
    });
}

#[gpui_kit::test]
fn command_k_browses_a_menu_when_no_object_actions_exist(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, _window, cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::Launcher;
            view.model.rows = vec![LauncherRow {
                id: SharedString::from("menu-item"),
                title: SharedString::from("Menu item"),
                subtitle: None,
                shortcut: None,
                icon: "menu".to_owned(),
                kind: CandidateKind::Action,
                enabled: true,
                is_menu: true,
                menu_path: vec!["View".to_owned()],
                command_id: None,
                file_kind: None,
                source_label: None,
                icon_identity: None,
                execution_context_id: None,
                primary_action: false,
            }]
            .into();
            view.model.selected = 0;
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, _, cx| {
        let view = view.read(cx);
        assert!(!view.model.object_actions_active);
        assert_eq!(view.model.menu_path(), Some(&["View".to_owned()][..]));
    })
    .unwrap();
}

#[gpui_kit::test]
fn launcher_height_starts_compact_and_tracks_rows_and_scope_bar(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            assert_eq!(
                launcher_window_size().height,
                px(metrics::PAGE_HEADER_HEIGHT)
            );

            view.model.rows.clear();
            view.model.scopes.clear();
            let empty_height = view.desired_window_size().height;
            assert_eq!(empty_height, px(72.));

            view.model.scopes.push(crate::launcher::LauncherScope {
                id: "current-app".to_owned(),
                label: "Current App".to_owned(),
                is_current_app: true,
            });
            assert_eq!(
                view.desired_window_size().height,
                empty_height,
                "a single Current App scope belongs in the header"
            );

            view.model.scopes.push(crate::launcher::LauncherScope {
                id: "files".to_owned(),
                label: "Files".to_owned(),
                is_current_app: false,
            });
            assert_eq!(view.desired_window_size().height, px(108.5));

            view.model.scopes.clear();
            view.model.rows = (0..5)
                .map(|index| file_row(&format!("row-{index}"), "Result"))
                .collect();
            assert_eq!(view.desired_window_size().height, px(360.5));

            view.model.rows = (0..7)
                .map(|index| file_row(&format!("row-{index}"), "Result"))
                .collect();
            assert_eq!(view.desired_window_size().height, px(472.5));

            view.model.rows = (0..8)
                .map(|index| file_row(&format!("row-{index}"), "Result"))
                .collect();
            assert_eq!(view.desired_window_size().height, px(520.));
        });
    });
}

#[gpui_kit::test]
fn launcher_resize_converges_for_loading_empty_and_changing_result_counts(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.set_query("fixture".to_owned());
            view.model.rows = (0..5)
                .map(|index| file_row(&format!("row-{index}"), "Result"))
                .collect();
            view.model.loading = false;
            cx.notify();
        });
    });
    render_and_sync_test_viewport(cx, window, &view);
    assert_eq!(
        cx.update_window(window, |_, window, _| window.viewport_size().height)
            .unwrap(),
        px(360.5)
    );

    for rows in [1, 5] {
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.rows = (0..rows)
                    .map(|index| file_row(&format!("row-{index}"), "Result"))
                    .collect();
                view.model.selected = 0;
                cx.notify();
            });
        });
        render_and_sync_test_viewport(cx, window, &view);
        let expected = if rows == 1 { 136.5 } else { 360.5 };
        assert_eq!(
            cx.update_window(window, |_, window, _| window.viewport_size().height)
                .unwrap(),
            px(expected)
        );
    }

    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.rows.clear();
            view.model.loading = true;
            cx.notify();
        });
    });
    render_and_sync_test_viewport(cx, window, &view);
    assert_eq!(
        cx.update_window(window, |_, window, _| window.viewport_size().height)
            .unwrap(),
        px(136.5)
    );

    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.loading = false;
            view.model.set_query(String::new());
            view.model.rows.clear();
            view.clear_notice();
            cx.notify();
        });
    });
    render_and_sync_test_viewport(cx, window, &view);
    assert_eq!(
        cx.update_window(window, |_, window, _| window.viewport_size().height)
            .unwrap(),
        px(72.)
    );
}

#[gpui_kit::test]
fn preview_query_without_a_runtime_resizes_to_its_local_empty_state(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("launcher-search-input", cx);
        window.input("no-preview-match-zzzz", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let (bounds, desired, rows) = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            (
                window.bounds().size,
                view.read(cx).desired_window_size(),
                view.read(cx).model.rows.len(),
            )
        })
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(
        bounds, desired,
        "preview filtering must trigger the same native size update"
    );
    sync_test_viewport(cx, window, desired);
    assert_eq!(
        cx.update_window(window, |_, window, _| window.viewport_size().height)
            .unwrap(),
        desired.height
    );
}

#[gpui_kit::test]
fn constrained_settings_viewport_fills_the_native_window_and_keeps_content_inside(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::General;
            cx.notify();
        });
    });
    let constrained = gpui_kit::size(px(648.), px(480.));
    sync_test_viewport(cx, window, constrained);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let surface = window.find("settings-surface").bounds();
        assert_eq!(surface.size.height, constrained.height);
        let content = window.find("settings-content-scroll-general").bounds();
        assert!(content.bottom() <= constrained.height);
        assert!(
            view.read(cx).settings_editor.scrolls[SettingsCategory::General as usize]
                .max_offset()
                .y
                >= px(0.)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn current_app_moves_from_the_header_to_the_scope_bar(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-current-app-label").is_some());
        assert!(window.try_find("scope-all").is_none());

        view.update(cx, |view, cx| {
            view.model.scopes = vec![crate::launcher::LauncherScope {
                id: "current-app".to_owned(),
                label: "Current App".to_owned(),
                is_current_app: true,
            }];
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("launcher-current-app-label").is_some());
        assert!(window.try_find("scope-all").is_none());

        view.update(cx, |view, cx| {
            view.model.scopes.push(crate::launcher::LauncherScope {
                id: "files".to_owned(),
                label: "Files".to_owned(),
                is_current_app: false,
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("launcher-current-app-label").is_none());
        assert!(window.try_find("scope-all").is_none());
        assert!(window.try_find("scope-files").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn search_mode_chooser_expands_inside_the_window_and_applies_keyboard_choice(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("launcher-mode-toggle").expanded(), Some(false));
        window.click("launcher-mode-toggle", cx);
    })
    .unwrap();
    cx.run_until_parked();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-mode-chooser").is_some());
        assert_eq!(window.find("launcher-mode-toggle").expanded(), Some(true));
        assert_eq!(
            window.find("launcher-navigation-environment").selected(),
            Some(true)
        );
        assert_eq!(
            window.find("launcher-navigation-search").selected(),
            Some(false)
        );
        view.update(cx, |view, _| {
            assert_eq!(view.desired_window_size().height, px(180.5));
        });
        window.press("down", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("launcher-navigation-search").selected(),
            Some(true)
        );
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-mode-chooser").is_none());
        view.update(cx, |view, _| {
            assert_eq!(view.input_mode, InputMode::Search);
            assert!(!view.root_navigation_open);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn mode_chooser_resizes_compact_window_and_keeps_both_languages_inside_rows(
    cx: &mut TestAppContext,
) {
    let (handle, view) = open_view(cx);
    for locale in [Locale::En, Locale::Zh] {
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.locale = locale;
                view.root_navigation_open = false;
                view.model.rows.clear();
                view.model.scopes.clear();
                view.model.loading = false;
                view.clear_notice();
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, handle, &view);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("launcher-mode-toggle", cx);
        })
        .unwrap();
        cx.run_until_parked();
        sync_test_viewport_to_desired(cx, handle, &view);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.viewport_size().height, px(180.5));
            let header = window.find("launcher-search-header").bounds();
            let chooser = window.find("launcher-mode-chooser").bounds();
            assert!(chooser.top() >= header.bottom());
            assert!(chooser.bottom() <= px(180.5));
            for id in [
                "launcher-navigation-search",
                "launcher-navigation-environment",
            ] {
                let row = window.find(id).bounds();
                let title = window.find(format!("{id}-title")).bounds();
                let detail = window.find(format!("{id}-detail")).bounds();
                assert_eq!(row.size.height, px(metrics::ROW_HEIGHT_COMPACT));
                assert!(title.top() >= row.top());
                assert!(detail.top() >= title.bottom());
                assert!(detail.bottom() <= row.bottom());
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn long_candidate_text_does_not_compress_rows_or_cover_primary_action(cx: &mut TestAppContext) {
    let (handle, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.rows = (0..20)
                .map(|i| {
                    let mut row =
                        file_row(&format!("long-{i}"), &"长文件名称 with spaces ".repeat(15));
                    row.subtitle = Some(SharedString::from("/very/long/目录/path/".repeat(30)));
                    row.shortcut = Some("⌃⌥⇧⌘F12".into());
                    row.enabled = true;
                    row
                })
                .collect();
            view.model.selected = 0;
            view.model.loading = false;
            view.clear_notice();
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let first = window.find("candidate-0").bounds();
        let second = window.find("candidate-1").bounds();
        let title = window.find("candidate-0-title").bounds();
        let subtitle = window.find("candidate-0-subtitle").bounds();
        let action = window.find("candidate-0-primary").bounds();
        let input = window.find("launcher-search-input-slot").bounds();
        assert_eq!(first.size.height, px(LAUNCHER_ROW_HEIGHT));
        assert_eq!(second.size.height, px(LAUNCHER_ROW_HEIGHT));
        assert_eq!(title.left(), input.left());
        assert!(first.bottom() <= second.top());
        assert!(title.right() <= action.left());
        assert!(subtitle.right() <= action.left());
        assert!(title.bottom() <= subtitle.top());
        assert!(subtitle.bottom() <= first.bottom());
    })
    .unwrap();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let header = window.find("file-search-header").bounds();
        let filters = window.find("file-search-controls").bounds();
        assert_eq!(header.size.height, px(metrics::PAGE_HEADER_HEIGHT));
        assert_eq!(filters.size.height, px(metrics::PAGE_TOOLBAR_HEIGHT));
        assert!(filters.top() >= header.bottom());
        assert_eq!(
            window.find("candidate-0").bounds().size.height,
            px(LAUNCHER_ROW_HEIGHT)
        );
        assert!(window.find("candidate-0").bounds().top() >= filters.bottom());
    })
    .unwrap();
}

#[gpui_kit::test]
fn list_rows_across_pages_keep_height_title_alignment_and_non_overlapping_actions(
    cx: &mut TestAppContext,
) {
    let (handle, view) = open_view(cx);
    let long_title = "长条目名称 with spaces ".repeat(12);
    let long_subtitle = "/very/long/目录/subpath/".repeat(20);

    // Clipboard
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: true,
                recording_enabled: true,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 2)),
                notice: None,
                items: vec![
                    crate::host::ClipboardItem {
                        id: 1,
                        summary: long_title.clone(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 10,
                    },
                    crate::host::ClipboardItem {
                        id: 2,
                        summary: long_title.clone(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 10,
                    },
                ],
            });
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let first = window.find("clipboard-item-1").bounds();
        let second = window.find("clipboard-item-2").bounds();
        let title0 = window.find("clipboard-title-1").bounds();
        let title1 = window.find("clipboard-title-2").bounds();
        let subtitle0 = window.find("clipboard-item-1-subtitle").bounds();
        let action = window.find("clipboard-copy-1").bounds();
        assert_eq!(
            first.size.height,
            px(crate::ui::design_tokens::metrics::ROW_HEIGHT_COMFORTABLE)
        );
        assert_eq!(title0.left(), title1.left());
        assert!(first.bottom() <= second.top());
        assert!(title0.right() <= action.left());
        assert!(subtitle0.right() <= action.left());
        assert!(subtitle0.bottom() <= first.bottom());
    })
    .unwrap();

    // Clipboard Shelf
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let first = window.find("clipboard-shelf-item-1").bounds();
        let second = window.find("clipboard-shelf-item-2").bounds();
        let title0 = window.find("shelf-title-1").bounds();
        let title1 = window.find("shelf-title-2").bounds();
        let subtitle0 = window.find("clipboard-shelf-item-1-subtitle").bounds();
        let action = window.find("shelf-more-1").bounds();
        let copy = window.find("shelf-copy-1").bounds();
        assert!(action.right() <= copy.left());
        assert!(copy.right() <= first.right());
        assert_eq!(
            first.size.height,
            px(crate::ui::design_tokens::metrics::ROW_HEIGHT_COMPACT)
        );
        assert_eq!(title0.left(), title1.left());
        assert!(first.bottom() <= second.top());
        assert!(title0.right() <= action.left());
        assert!(subtitle0.right() <= action.left());
        assert!(subtitle0.bottom() <= first.bottom());
    })
    .unwrap();

    // Application History
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.apply(crate::host::AppHistorySnapshot {
                source_id: "code".to_owned(),
                items: vec![
                    crate::host::AppHistoryItem {
                        id: "h1".to_owned(),
                        object_key: "code:h1".to_owned(),
                        source_id: "code".to_owned(),
                        title: long_title.clone(),
                        value: long_subtitle.clone(),
                        kind: "directory",
                        can_open: true,
                    },
                    crate::host::AppHistoryItem {
                        id: "h2".to_owned(),
                        object_key: "code:h2".to_owned(),
                        source_id: "code".to_owned(),
                        title: long_title.clone(),
                        value: long_subtitle.clone(),
                        kind: "directory",
                        can_open: true,
                    },
                ],
                failed_sources: vec![],
                notice: None,
            });
            view.app_history.selected = 0;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let first = window.find("history-item-h1").bounds();
        let second = window.find("history-item-h2").bounds();
        let title0 = window.find("history-item-h1-title").bounds();
        let title1 = window.find("history-item-h2-title").bounds();
        let subtitle0 = window.find("history-item-h1-subtitle").bounds();
        let action = window.find("history-open-h1").bounds();
        assert_eq!(
            first.size.height,
            px(crate::ui::design_tokens::metrics::ROW_HEIGHT_COMFORTABLE)
        );
        assert_eq!(title0.left(), title1.left());
        assert!(first.bottom() <= second.top());
        assert!(title0.right() <= action.left());
        assert!(subtitle0.right() <= action.left());
        assert!(subtitle0.bottom() <= first.bottom());
    })
    .unwrap();

    // Web Packages
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebPackages;
            view.web.apply(vec![
                crate::host::WebPackage {
                    id: "pkg-a".into(),
                    name: long_title.clone(),
                    version: "1.0.0".into(),
                    digest: "abc".into(),
                    entry: "index.html".into(),
                    enabled: true,
                    available: true,
                    rollback: false,
                    builtin: false,
                    permission_count: 1,
                },
                crate::host::WebPackage {
                    id: "pkg-b".into(),
                    name: long_title.clone(),
                    version: "1.0.0".into(),
                    digest: "def".into(),
                    entry: "index.html".into(),
                    enabled: false,
                    available: true,
                    rollback: false,
                    builtin: false,
                    permission_count: 1,
                },
            ]);
            view.web.selected = 0;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let first = window.find("web-package-0").bounds();
        let second = window.find("web-package-1").bounds();
        let title0 = window.find("web-package-0-title").bounds();
        let title1 = window.find("web-package-1-title").bounds();
        let subtitle0 = window.find("web-package-0-subtitle").bounds();
        let action = window.find("web-toggle-slot-pkg-a").bounds();
        assert_eq!(
            first.size.height,
            px(crate::ui::design_tokens::metrics::ROW_HEIGHT_COMFORTABLE)
        );
        assert_eq!(title0.left(), title1.left());
        assert!(first.bottom() <= second.top());
        assert!(title0.right() <= action.left());
        assert!(subtitle0.right() <= action.left());
        assert!(subtitle0.bottom() <= first.bottom());
    })
    .unwrap();
}

#[gpui_kit::test]
fn file_search_selection_reveals_rows_in_a_long_tracked_list(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.rows = (0..20)
                .map(|index| file_row(&format!("file-{index}"), &format!("file-{index}.txt")))
                .collect();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.model.loading = false;
            view.notice = None;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let initial_maximum = view.read(cx).scrolls.file_search_base().max_offset().y;
        assert!(
            initial_maximum > px(0.),
            "a long result list must expose a real scroll range"
        );
        view.update(cx, |view, cx| {
            for _ in 0..19 {
                view.select_file_search(1, cx);
            }
            assert_eq!(view.model.selected, 19);
        });
        window.render_frame(cx);
        let scroll = &view.read(cx).scrolls.file_search_base();
        assert!(
            scroll.offset().y < px(0.),
            "revealing the selected row must scroll the tracked list"
        );
        let selected = window.find("candidate-19").bounds();
        assert!(selected.bottom() <= scroll.bounds().bottom());
    })
    .unwrap();
}

#[gpui_kit::test]
fn tasks_output_follow_tracks_the_shared_scroll_handle(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "streaming".to_owned(),
        title: "Streaming output".to_owned(),
        directory: "/tmp/fixture".to_owned(),
        command: "printf output".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 1,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.tasks.apply_rows(vec![row.clone()]);
            view.tasks.detail = Some(crate::host::ExecutionDetail {
                row,
                stdout: (0..200)
                    .map(|index| format!("output line {index}"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into(),
                stderr: "".into(),
            });
            view.tasks.request_follow_output();
            view.notice = None;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(window, |_, _, cx| {
        let scroll = &view.read(cx).scrolls.tasks_output_base();
        let maximum = scroll.max_offset().y;
        assert!(
            maximum > px(0.),
            "long task output must expose a real scroll range"
        );
        assert_eq!(scroll.offset().y, -maximum);
        assert!(view.read(cx).tasks.follow_output);
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_shortcut_closes_mode_chooser_and_ignores_queued_launcher_resize(
    cx: &mut TestAppContext,
) {
    let (handle, view) = open_view(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("launcher-mode-toggle", cx);
        window.press(&primary_shortcut(","), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(view.read(cx).screen, Screen::Settings);
        assert!(!view.read(cx).root_navigation_open);
        assert_eq!(window.bounds().size.height, px(SETTINGS_HEIGHT));
        assert!(window.try_find("settings-surface").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selected_file_row_exposes_the_original_preview_affordance(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.file_search.browsing = true;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("candidate-0-preview").is_some());
        assert!(window.try_find("candidate-0-actions").is_none());
        assert!(window.try_find("candidate-0-primary").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn menu_navigation_replaces_the_mode_control_with_back(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.rows = vec![LauncherRow {
                id: SharedString::from("menu-item"),
                title: SharedString::from("Menu item"),
                subtitle: None,
                shortcut: None,
                icon: "menu".to_owned(),
                kind: CandidateKind::Action,
                enabled: true,
                is_menu: true,
                menu_path: Vec::new(),
                command_id: None,
                file_kind: None,
                source_label: None,
                icon_identity: None,
                execution_context_id: None,
                primary_action: false,
            }]
            .into();
            assert!(view.model.open_selected_menu());
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-back").is_some());
        assert!(window.try_find("launcher-mode-toggle").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn only_the_transient_main_launcher_dismisses_on_focus_loss(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            assert!(!view.dismiss_on_focus_loss());
            view.presence.presented = true;
            assert!(view.dismiss_on_focus_loss());

            view.screen = Screen::FileSearch;
            assert!(view.dismiss_on_focus_loss());

            view.preview.open = true;
            assert!(!view.dismiss_on_focus_loss());
            view.preview.open = false;
            view.preview.busy = true;
            assert!(!view.dismiss_on_focus_loss());
            view.preview.busy = false;

            view.screen = Screen::Settings;
            assert!(!view.dismiss_on_focus_loss());

            view.screen = Screen::WebPackages;
            assert!(!view.dismiss_on_focus_loss());

            view.screen = Screen::Launcher;
            view.standalone_shelf = true;
            assert!(!view.dismiss_on_focus_loss());
        });
    });
}

#[gpui_kit::test]
fn reopening_content_focuses_the_visible_editor(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Launcher;
            view.input_mode = InputMode::Content;
            view.input.update(cx, |input, cx| input.focus(window, cx));
            assert!(!view.active_editor_focused(window, cx));
            view.refocus_active_editor(window, cx);
            assert!(view
                .content_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
            assert!(view.active_editor_focused(window, cx));
            view.input_mode = InputMode::Search;
            view.refocus_active_editor(window, cx);
            assert!(view.input.read(cx).focus_handle(cx).is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn clipboard_confirmation_blocks_shortcuts_and_keeps_its_target(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((2, 2)),
                notice: None,
                items: (1..=2)
                    .map(|id| crate::host::ClipboardItem {
                        id,
                        summary: id.to_string(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 1,
                    })
                    .collect(),
            });
            view.request_delete_clipboard_item(1, cx);
            view.handle_key(
                "down",
                KeyModifiers {
                    command: true,
                    modified: true,
                    ..Default::default()
                },
                window,
                cx,
            );
            assert_eq!(view.clipboard.selected_item().unwrap().id, 1);
            view.clipboard.selected = 1; // A refreshed snapshot changes selection.
            view.confirm_clipboard_delete(cx);
            assert_eq!(view.clipboard.selected_item().unwrap().id, 1);
            view.clipboard.status.as_mut().unwrap().items.remove(0);
            view.clipboard.selected = 0;
            view.confirm_clipboard_delete(cx); // Missing target must not authorize item 2.
            assert_eq!(view.clipboard.operation_generation, 0);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.clipboard.delete_pending.is_none());
            view.clipboard.delete_pending = Some(1);
            view.screen = Screen::Launcher;
            view.root_navigation_open = true;
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.root_navigation_open);
            assert_eq!(view.clipboard.delete_pending, Some(1));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn hiding_the_shelf_invalidates_pending_clipboard_operations(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.clipboard.operation_generation = 7;
            view.prepare_hide(cx);
            assert_eq!(view.clipboard.operation_generation, 8);
        });
    });
}

#[gpui_kit::test]
fn invalid_context_binding_keeps_recording(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            for key in ["1", "c"] {
                view.shortcuts.recording = Some(ShortcutSlot::ContextShortcut);
                view.handle_key(
                    key,
                    KeyModifiers {
                        command: true,
                        modified: true,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(
                    view.shortcuts.recording,
                    Some(ShortcutSlot::ContextShortcut)
                );
                assert!(view.notice.is_some());
            }
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn modified_navigation_preserves_history_selection(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.items = (0..3)
                .map(|index| crate::host::AppHistoryItem {
                    id: index.to_string(),
                    object_key: index.to_string(),
                    source_id: "code".into(),
                    title: "Project".into(),
                    value: "/tmp/project".into(),
                    kind: "directory",
                    can_open: true,
                })
                .collect();
            for modifiers in [
                KeyModifiers {
                    shift: true,
                    modified: true,
                    ..Default::default()
                },
                KeyModifiers {
                    alt: true,
                    modified: true,
                    ..Default::default()
                },
                KeyModifiers {
                    command: true,
                    modified: true,
                    ..Default::default()
                },
                KeyModifiers {
                    control: true,
                    modified: true,
                    ..Default::default()
                },
            ] {
                view.handle_key("down", modifiers, window, cx);
                assert_eq!(view.app_history.selected, 0);
            }
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.selected, 1);
            view.handle_key("pageup", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.selected, 0);
            view.app_history.selected = 2;
            view.handle_key("pagedown", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.selected, 2);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn application_history_escape_clears_the_query_before_leaving(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.set_query("project".to_owned());
            set_input_value(&view.history_input, "project", window, cx);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::ApplicationHistory);
            assert_eq!(view.history_input.read(cx).value(), "");
            assert!(view.app_history.query.is_empty());
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::Launcher);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn application_history_escape_control_uses_the_same_query_chain(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.set_query("project".to_owned());
            set_input_value(&view.history_input, "project", window, cx);
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("page-escape", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::ApplicationHistory);
        assert_eq!(view.history_input.read(cx).value(), "");
        assert!(view.app_history.query.is_empty());
    });
}

#[gpui_kit::test]
fn opening_application_history_starts_with_a_fresh_query_and_rows(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.app_history.set_query("old query".to_owned());
            view.app_history.items = vec![crate::host::AppHistoryItem {
                id: "old".to_owned(),
                object_key: "old".to_owned(),
                source_id: "old-source".to_owned(),
                title: "Old source".to_owned(),
                value: "/tmp/old".to_owned(),
                kind: "file",
                can_open: true,
            }];
            set_input_value(&view.history_input, "old query", window, cx);
            view.screen = Screen::Launcher;
            view.open_screen(Screen::ApplicationHistory, window, cx);

            assert_eq!(view.screen, Screen::ApplicationHistory);
            assert_eq!(view.history_input.read(cx).value(), "");
            assert!(view.app_history.query.is_empty());
            assert!(view.app_history.items.is_empty());
            assert!(view.app_history.loading);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn application_history_binds_command_k_to_actions(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.surface_focus_handle.focus(window, cx);
            view.app_history.items = vec![crate::host::AppHistoryItem {
                id: "remote".to_owned(),
                object_key: "remote:item".to_owned(),
                source_id: "browser".to_owned(),
                title: "Remote".to_owned(),
                value: "https://example.com".to_owned(),
                kind: "remote",
                can_open: false,
            }];
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, _| {
            assert!(view.app_history.fallback_actions_open);
            assert!(!view.app_history.object_actions_active);
            view.app_history.fallback_actions_open = false;
            view.app_history.items.clear();
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, _, cx| {
        assert!(view.read(cx).app_history.page_actions_open);
    })
    .unwrap();
}

#[gpui_kit::test]
fn application_history_binds_command_r_to_refresh(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.surface_focus_handle.focus(window, cx);
            view.app_history.session_generation = 7;
            view.app_history.query_generation = 11;
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-r");
    cx.update_window(window, |_, _, cx| {
        let view = view.read(cx);
        assert_eq!(view.app_history.session_generation, 7);
        assert_eq!(view.app_history.query_generation, 12);
    })
    .unwrap();
}

#[gpui_kit::test]
fn launcher_quick_index_shortcut_selects_and_activates_row(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.model.rows = (0..3)
                .map(|index| file_row(&format!("row-{index}"), &format!("Result {index}")))
                .collect();
            view.model.selected = 0;
            view.surface_focus_handle.focus(window, cx);
            cx.notify();
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-2");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(view.read(cx).model.selected, 1);
        assert!(window.try_find("launcher-footer").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn application_history_scrolls_selected_object_actions_and_dismisses(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.apply(crate::host::AppHistorySnapshot {
                source_id: "code".to_owned(),
                items: vec![crate::host::AppHistoryItem {
                    id: "item".to_owned(),
                    object_key: "code:item".to_owned(),
                    source_id: "code".to_owned(),
                    title: "Project".to_owned(),
                    value: "/tmp/project".to_owned(),
                    kind: "directory",
                    can_open: true,
                }],
                failed_sources: vec![],
                notice: None,
            });
            let request = view
                .app_history
                .begin_object_actions_request("item".to_owned(), view.model.revision);
            assert!(view.app_history.apply_object_actions(
                request,
                "item",
                view.model.revision,
                (0..18)
                    .map(|index| crate::host::AppHistoryObjectAction {
                        id: format!("action-{index}"),
                        title: format!("Action {index}"),
                        localized_titles: Default::default(),
                        enabled: true,
                    })
                    .collect(),
            ));
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    let viewport = cx
        .update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("history-object-action-0").is_some());
            assert!(window.try_find("history-object-action-refresh").is_some());
            window.viewport_size()
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    let panel = visual
        .debug_bounds("history-object-actions")
        .expect("history object actions panel bounds");
    assert_eq!(panel.size.width, viewport.width);
    let menu_scroll = visual.update(|_, cx| view.read(cx).scrolls.history_actions.clone());
    assert!(menu_scroll.max_offset().y > px(0.));
    visual.update(|window, _| assert!(window.try_find("history-item-item").is_none()));
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.handle_key("up", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.object_action_selected, 18);
        });
        window.render_frame(cx);
        assert!(menu_scroll.offset().y < px(0.));
        let refreshed = window.find("history-object-action-refresh").bounds();
        assert!(refreshed.top() >= menu_scroll.bounds().top());
        assert!(refreshed.bottom() <= menu_scroll.bounds().bottom());
    });
    assert!(
        panel.left() >= px(0.) && panel.right() >= viewport.width - px(16.),
        "panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        panel.bottom() <= viewport.height,
        "panel={panel:?} viewport={viewport:?}"
    );
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.handle_key(
                "escape",
                KeyModifiers {
                    command: false,
                    control: false,
                    shift: false,
                    alt: false,
                    modified: false,
                },
                window,
                cx,
            );
        });
        window.render_frame(cx);
        assert!(window.try_find("history-object-action-0").is_none());
    });
}

#[gpui_kit::test]
fn application_history_object_actions_keep_copy_and_refresh_fallbacks(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.apply(crate::host::AppHistorySnapshot {
                source_id: "code".to_owned(),
                items: vec![crate::host::AppHistoryItem {
                    id: "item".to_owned(),
                    object_key: "code:item".to_owned(),
                    source_id: "code".to_owned(),
                    title: "Project".to_owned(),
                    value: "/tmp/project".to_owned(),
                    kind: "directory",
                    can_open: true,
                }],
                failed_sources: vec![],
                notice: None,
            });
            let request = view
                .app_history
                .begin_object_actions_request("item".to_owned(), view.model.revision);
            assert!(view.app_history.apply_object_actions(
                request,
                "item",
                view.model.revision,
                Vec::new(),
            ));
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("history-object-action-copy").is_some());
        assert!(window.try_find("history-object-action-refresh").is_some());
        view.update(cx, |view, cx| {
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.object_action_selected, 1);
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.object_action_selected, 0);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn empty_application_history_exposes_refresh_actions(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.items.clear();
            view.app_history.page_actions_open = false;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("app-history-page-actions").is_some());
        window.click("app-history-page-actions", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("app-history-page-refresh").is_some());
        view.update(cx, |view, cx| {
            view.history_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!view.read(cx).app_history.page_actions_open);
    });
}

#[gpui_kit::test]
fn non_openable_application_history_exposes_copy_and_refresh(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.items = vec![crate::host::AppHistoryItem {
                id: "remote".to_owned(),
                object_key: "remote:item".to_owned(),
                source_id: "browser".to_owned(),
                title: "Remote".to_owned(),
                value: "https://example.com".to_owned(),
                kind: "remote",
                can_open: false,
            }];
            view.app_history.selected = 0;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("app-history-page-actions", cx);
        window.render_frame(cx);
        assert!(window.try_find("app-history-page-refresh").is_some());
        view.update(cx, |view, cx| {
            view.handle_key("escape", KeyModifiers::default(), window, cx);
        });
        window.render_frame(cx);
        window.click("history-actions-remote", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window
            .try_find("app-history-fallback-actions-panel")
            .is_some());
        assert!(window.try_find("app-history-fallback-action-0").is_some());
        assert!(window.try_find("app-history-fallback-action-1").is_some());
        assert!(window.try_find("app-history-page-refresh").is_none());
        view.update(cx, |view, cx| {
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.app_history.fallback_action_selected, 1);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.app_history.fallback_actions_open);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn clipboard_actions_panel_stays_inside_the_page(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let viewport = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Clipboard;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: true,
                    recording_enabled: true,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    storage_usage: Some((1, 4)),
                    notice: None,
                    items: vec![crate::host::ClipboardItem {
                        id: 1,
                        summary: "fixture".into(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 4,
                    }],
                });
                view.clipboard.browsing = true;
                view.clipboard.actions_open = true;
                view.clipboard.action_target = Some(1);
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("clipboard-action-row-0").is_some());
            window.viewport_size()
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    let panel = visual
        .debug_bounds("clipboard-actions")
        .expect("clipboard actions panel bounds");
    assert_eq!(panel.size.width, viewport.width);
    assert!(
        panel.left() >= px(0.) && panel.right() >= viewport.width - px(16.),
        "panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        panel.bottom() <= viewport.height,
        "panel={panel:?} viewport={viewport:?}"
    );
}

#[gpui_kit::test]
fn empty_clipboard_pages_expose_refresh_without_a_target(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.status = None;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("clipboard-page-actions").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.open_empty_clipboard_actions(cx);
            assert!(view.clipboard.actions_open);
            assert_eq!(view.clipboard.action_target, None);
            assert_eq!(view.clipboard.action_selected, 3);
            view.move_clipboard_action(-1, 4, cx);
            assert_eq!(view.clipboard.action_selected, 3);
            view.move_clipboard_action(1, 4, cx);
            assert_eq!(view.clipboard.action_selected, 3);
            view.execute_clipboard_action(3, cx);
            assert!(!view.clipboard.actions_open);

            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.open_empty_clipboard_actions(cx);
            assert_eq!(view.clipboard.action_selected, 4);
            view.move_clipboard_action(-1, 5, cx);
            assert_eq!(view.clipboard.action_selected, 4);
            view.move_clipboard_action(1, 5, cx);
            assert_eq!(view.clipboard.action_selected, 4);
            view.execute_clipboard_action(4, cx);
            assert!(!view.clipboard.actions_open);
            assert_eq!(view.clipboard.action_selected, 0);
        });
    });
}

#[gpui_kit::test]
fn tasks_tool_options_stay_inside_the_page(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let viewport = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.tasks.tool_options_open = true;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                view.read(cx).tasks.tool_options_open,
                "the render must preserve the open tool options state"
            );
            window.viewport_size()
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, _| {
        assert!(window.try_find("page-back").is_some());
        assert!(window.try_find("tasks-refresh").is_some());
    });
    let panel = visual
        .debug_bounds("tasks-tool-options-panel")
        .expect("tasks tool options panel bounds");
    assert_eq!(panel.size.width, viewport.width);
    assert!(
        panel.left() >= px(0.) && panel.right() >= viewport.width - px(22.),
        "panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        panel.top() >= px(metrics::TITLE_BAR_INSET) && panel.bottom() <= viewport.height,
        "panel={panel:?} viewport={viewport:?}"
    );
}

#[gpui_kit::test]
fn tasks_actions_stay_inside_the_page(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "npm Run · task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "npm run task".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
        output_revision: 1,
        exit_code: Some(0),
        started_at_ms: 0,
        finished_at_ms: Some(1),
        output_truncated: false,
        error: None,
    };
    let viewport = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.tasks.apply_rows(vec![row.clone()]);
                view.tasks.detail = Some(crate::host::ExecutionDetail {
                    row,
                    stdout: "ready".into(),
                    stderr: "".into(),
                });
                view.tasks.task_actions_open = true;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("task-action-copy-command").is_some());
            assert!(window.try_find("task-more-actions").is_none());
            window.viewport_size()
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    let panel = visual
        .debug_bounds("tasks-actions")
        .expect("tasks actions panel bounds");
    assert_eq!(panel.size.width, viewport.width);
    assert!(
        panel.left() >= px(0.) && panel.right() >= viewport.width - px(22.),
        "panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        panel.top() >= px(metrics::TITLE_BAR_INSET) && panel.bottom() <= viewport.height,
        "panel={panel:?} viewport={viewport:?}"
    );
}

#[gpui_kit::test]
fn tasks_details_hide_windows_verbatim_path_prefixes(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "npm Run · sample".to_owned(),
        directory: r"\\?\C:\Users\fixture\project".to_owned(),
        command: "npm run sample".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
        output_revision: 1,
        exit_code: Some(0),
        started_at_ms: 0,
        finished_at_ms: Some(1),
        output_truncated: false,
        error: None,
    };
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.tasks.apply_rows(vec![row.clone()]);
            view.tasks.detail = Some(crate::host::ExecutionDetail {
                row,
                stdout: "ready".into(),
                stderr: "".into(),
            });
            view.tasks.command_details_open = true;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("task-detail-directory").label(),
            Some(r"C:\Users\fixture\project")
        );
        assert_eq!(
            window.find("task-command-directory").label(),
            Some(r"C:\Users\fixture\project")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn search_updates_rows_and_keyboard_selection_in_the_real_input(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("launcher-search-input", cx);
        window.input("Git", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert_eq!(view.model.query, "Git");
        assert_eq!(view.model.rows.len(), 1);
        assert_eq!(view.model.selected_row().unwrap().id, "git-status");
        assert_eq!(
            window.find("candidate-0").label(),
            Some("Git Status"),
            "candidate rows expose their semantic label"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn file_search_requires_navigation_before_actions_and_space_preview(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![
                file_row("file-a", "a.txt"),
                file_row("file-b", "b.txt"),
                file_row("file-c", "c.txt"),
            ]
            .into();
            view.model.selected = 0;
            view.file_search.browsing = false;

            assert!(view.selected_row_for_action().is_none());
            assert!(!view.file_search_selection_active());
            assert!(!view.can_preview_selected_file());
            view.activate_selection(cx);
            view.show_object_actions(cx);
            assert!(view.notice.is_none());
            assert!(!view.model.object_actions_active);

            view.select_file_search(1, cx);
            assert!(view.file_search.browsing);
            assert_eq!(view.model.selected, 0);
            assert_eq!(
                view.selected_row_for_action()
                    .as_deref()
                    .map(|row| row.id.as_str()),
                Some("file-a")
            );
            assert!(view.can_preview_selected_file());

            view.reset_file_search_browsing(cx);
            assert!(!view.file_search.browsing);
            view.select_file_search(-1, cx);
            assert_eq!(view.model.selected, 0);
        });
    });
}

#[gpui_kit::test]
fn file_search_space_only_previews_after_navigation_and_escape_closes_preview(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = false;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("file-search-input", cx);
        window.input("guide", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.input.read(cx).value().as_ref(), "guide");
        assert_eq!(view.model.query, "guide");
    });

    cx.update_window(window, |_, window, cx| {
        window.press("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(
            view.input.read(cx).value().as_ref(),
            "guide ",
            "space remains query text before the file list is browsed"
        );
        assert_eq!(view.model.query, "guide ");
    });

    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.file_search.browsing = true;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
        });
        assert!(view.read(cx).file_search_selection_active());
    });
    cx.update_window(window, |_, window, cx| {
        window.press("space", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).input.read(cx).value().as_ref(),
            "guide ",
            "space preview must not be inserted into the query"
        );
        assert_eq!(view.read(cx).model.query, "guide ");
    });

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input
                .update(cx, |input, cx| input.insert(" ", window, cx));
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(
            view.input.read(cx).value().as_ref(),
            "guide ",
            "an IME-delivered space must be rolled back before previewing"
        );
        assert_eq!(view.model.query, "guide ");
        assert!(view.file_search.browsing);
    });

    cx.update(|cx| {
        view.update(cx, |view, _| view.preview.open = true);
    });
    cx.update_window(window, |_, window, cx| {
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.preview.open);
        assert_eq!(view.screen, Screen::FileSearch);
        assert_eq!(
            view.input.read(cx).value().as_ref(),
            "guide ",
            "escape closes preview before changing the query"
        );
    });

    cx.update(|cx| {
        view.update(cx, |view, _| view.file_search.browsing = true);
    });
    cx.update_window(window, |_, window, cx| {
        window.input("x", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.file_search.browsing);
        assert_eq!(view.model.selected, 0);
        assert_eq!(view.model.query, "guide x");
    });
}

#[gpui_kit::test]
fn leaving_settings_keeps_resumes_or_restarts_the_launcher_stream(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.live_state.session_id = Some(7);
            view.presence.retained_workspace = true;
            view.presence.hidden_at = Some(Instant::now());
            assert_eq!(
                view.settings_launcher_recovery(),
                LauncherStreamRecovery::Resume
            );

            let (cancel, _cancelled) = oneshot::channel();
            view.live_state.cancel = Some(cancel);
            assert_eq!(
                view.settings_launcher_recovery(),
                LauncherStreamRecovery::KeepActive
            );

            view.live_state.cancel = None;
            view.presence.hidden_at = Some(
                Instant::now()
                    - Duration::from_millis(anywhere_protocol::policy::LAUNCHER_RETENTION_MS + 1),
            );
            assert_eq!(
                view.settings_launcher_recovery(),
                LauncherStreamRecovery::Restart
            );

            view.presence.hidden_at = None;
            view.presence.retained_workspace = false;
            view.live_state.session_id = None;
            assert_eq!(
                view.settings_launcher_recovery(),
                LauncherStreamRecovery::Restart
            );
        });
    });
}

#[gpui_kit::test]
fn settings_edits_are_dirty_until_saved_or_discarded(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| view.open_settings(window, cx));
        window.render_frame(cx);
        assert!(window.try_find("settings-surface").is_some());
        window.click("settings-category-general", cx);
        window.render_frame(cx);
        let login = window.find("open-at-login");
        assert_eq!(login.checked(), Some(false));
        window.click("open-at-login", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            assert!(view.settings_editor.draft.launch_at_login);
            assert!(!view.preferences.launch_at_login);
            assert!(view.settings_dirty(cx));
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("open-at-login").checked(), Some(true));
        window.click("save-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            assert!(view.preferences.launch_at_login);
            assert!(!view.settings_dirty(cx));
        });
    });

    cx.update_window(window, |_, window, cx| {
        window.click("open-at-login", cx);
        window.render_frame(cx);
        assert!(window.try_find("product-window-resize").is_none());
        if crate::platform::WINDOW_CONTROLS
            == crate::platform::WindowControlsStyle::TrailingCaptionButtons
        {
            let minimize = window.find("product-window-minimize").bounds();
            let close = window.find("product-window-close").bounds();
            assert!(minimize.right() <= close.left());
            assert!(close.left() > window.viewport_size().width / 2.0);
            window.click("product-window-close", cx);
        } else {
            assert!(window.try_find("product-window-close").is_none());
            view.update(cx, |view, cx| {
                assert!(!view.request_main_window_close(window, cx));
            });
        }
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.settings_editor.close_pending);
        assert_eq!(view.screen, Screen::Settings);
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("discard-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Launcher);
        assert!(
            !view.presence.presented,
            "closing Settings with the window close button must hide to background instead of presenting Launcher"
        );
        assert!(view.preferences.launch_at_login);
        assert!(view.settings_editor.draft.launch_at_login);
    });
}

#[gpui_kit::test]
fn settings_save_validation_error_stays_pinned_and_shows_in_close_prompt(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.open_settings(window, cx);
            view.settings_editor.model.selected = SettingsCategory::Search;
            set_textarea_value(
                &view.settings_editor.file_roots_input,
                String::new(),
                window,
                cx,
            );
            view.settings_editor.draft.file_search_roots.clear();
            view.settings_editor.draft.file_search_enabled = true;
            view.settings_editor.scrolls[SettingsCategory::Search as usize]
                .set_offset(gpui_kit::point(px(0.), px(-200.)));
            cx.notify();
        });
        window.render_frame(cx);
        window.click("save-settings", cx);
        window.render_frame(cx);
        let error_bounds = window.find("settings-save-error").bounds();
        let scroll_bounds = window.find("settings-content-scroll-search").bounds();
        assert!(
            error_bounds.bottom() <= scroll_bounds.top() + px(1.),
            "validation error must stay pinned above the scroll container even when scrolled"
        );
        assert!(view.read(cx).settings_editor.model.error.is_some());
    })
    .unwrap();

    cx.update_window(window, |_, window, cx| {
        window.click("done-settings", cx);
        window.render_frame(cx);
        assert!(view.read(cx).settings_editor.close_pending);
        window.click("save-settings-confirm", cx);
        window.render_frame(cx);
        assert!(view.read(cx).settings_editor.close_pending);
        assert!(window.try_find("settings-close-prompt-error").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_pointer_navigation_does_not_leave_general_keyboard_focused(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| view.open_settings(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view
            .read(cx)
            .settings_editor
            .focus_handle
            .is_focused(window));
        window.click("settings-category-shortcuts", cx);
        window.render_frame(cx);
        assert_eq!(
            view.read(cx).settings_editor.model.selected,
            SettingsCategory::Shortcuts
        );
        assert!(view
            .read(cx)
            .settings_editor
            .focus_handle
            .is_focused(window));
        assert_ne!(
            window.find("settings-category-general").focused(),
            Some(true)
        );
        view.update(cx, |view, cx| view.focus_settings_navigation(window, cx));
        assert!(view
            .read(cx)
            .settings_editor
            .focus_handle
            .is_focused(window));
    })
    .unwrap();
}

#[gpui_kit::test]
fn repeated_space_does_not_toggle_an_open_file_preview(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.preview.open = true;
        });
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.handle_preview_keystroke("space", false, false, true, window, cx));
            assert!(view.preview.open, "auto-repeat must not close the preview");
            assert!(view.handle_preview_keystroke("space", false, false, false, window, cx));
            assert!(!view.preview.open, "a new key-down closes the preview");
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn command_y_toggles_file_preview_once_and_ignores_repeat(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;

    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.preview.open = true;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.key_down(&primary_shortcut("y"), false, cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!view.read(cx).preview.open);
        view.update(cx, |view, _| {
            view.preview.open = true;
            crate::platform::set_key_event_is_repeat_for_test(true);
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.key_down(&primary_shortcut("y"), true, cx);
    })
    .unwrap();
    cx.update(|cx| {
        crate::platform::set_key_event_is_repeat_for_test(false);
        assert!(
            view.read(cx).preview.open,
            "held command-y must not toggle the preview twice"
        );
    });
}

#[gpui_kit::test]
fn command_y_closes_a_clipboard_preview_and_command_l_restores_search(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;

    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard_input
                .update(cx, |input, cx| input.focus(window, cx));
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 4)),
                notice: None,
                items: vec![crate::host::ClipboardItem {
                    id: 1,
                    summary: "fixture".into(),
                    kind: "text".into(),
                    copied_at_ms: 0,
                    byte_count: 4,
                }],
            });
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.preview.open = true;
        });
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        window.key_down(&primary_shortcut("y"), false, cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!view.read(cx).preview.open);
        view.update(cx, |view, _| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.preview.open = true;
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.key_down(&primary_shortcut("l"), false, cx);
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.preview.open);
        assert_eq!(view.screen, Screen::FileSearch);
    });
}

#[gpui_kit::test]
fn space_and_enter_activate_a_focused_page_button(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::Clipboard;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 4)),
                notice: None,
                items: vec![crate::host::ClipboardItem {
                    id: 1,
                    summary: "fixture".into(),
                    kind: "text".into(),
                    copied_at_ms: 0,
                    byte_count: 4,
                }],
            });
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.preview.open = true;
        });
    });
    let mut focused = false;
    for _ in 0..16 {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.focus_next(cx);
        })
        .unwrap();
        focused = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.find("page-back").focused() == Some(true)
            })
            .unwrap();
        if focused {
            break;
        }
    }
    assert!(focused, "the page back button must be reachable by Tab");
    cx.update_window(window, |_, window, cx| {
        window.press("space", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(
            view.screen,
            Screen::Launcher,
            "Space must activate the focused Back button"
        );
    });

    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::Clipboard;
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.preview.open = true;
        });
    });
    let mut focused = false;
    for _ in 0..16 {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.focus_next(cx);
        })
        .unwrap();
        focused = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.find("page-back").focused() == Some(true)
            })
            .unwrap();
        if focused {
            break;
        }
    }
    assert!(focused, "the page back button must be reachable by Tab");
    cx.update_window(window, |_, window, cx| {
        window.press("enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).screen,
            Screen::Launcher,
            "Enter must activate the focused Back button"
        );
    });
}

#[gpui_kit::test]
fn file_search_space_preview_requires_the_editor_to_own_focus(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.preview.open = true;

            window.blur(cx);
            assert!(!view.active_editor_focused(window, cx));
            assert!(!view.handle_preview_keystroke("space", false, false, false, window, cx));
            assert!(
                view.preview.open,
                "Space must not preview for an unfocused editor"
            );

            view.input.update(cx, |input, cx| input.focus(window, cx));
            assert!(view.active_editor_focused(window, cx));
            assert!(view.handle_preview_keystroke("space", false, false, false, window, cx));
            assert!(!view.preview.open);
        });
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).screen,
            Screen::FileSearch,
            "preview key ownership must not change the current page"
        );
    });
}

#[gpui_kit::test]
fn editing_navigation_closes_the_current_preview(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.browsing = true;
            view.preview.open = true;
            view.handle_key("left", KeyModifiers::default(), window, cx);
            assert!(!view.preview.open);
            assert!(!view.clipboard.browsing);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn repeated_space_edit_does_not_toggle_a_preview(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("file-a", "a.txt")].into();
            view.model.selected = 0;
            view.file_search.browsing = true;
            view.preview.open = true;
            crate::platform::set_key_event_is_repeat_for_test(true);
            view.input
                .update(cx, |input, cx| input.insert(" ", window, cx));
            assert!(view.preview.open);
            crate::platform::set_key_event_is_repeat_for_test(false);

            view.screen = Screen::Clipboard;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 4)),
                notice: None,
                items: vec![crate::host::ClipboardItem {
                    id: 1,
                    summary: "fixture".into(),
                    kind: "text".into(),
                    copied_at_ms: 0,
                    byte_count: 4,
                }],
            });
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.preview.open = true;
            crate::platform::set_key_event_is_repeat_for_test(true);
            view.clipboard_input
                .update(cx, |input, cx| input.insert(" ", window, cx));
            assert!(view.preview.open);
            crate::platform::set_key_event_is_repeat_for_test(false);
        });
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn settings_shortcut_recorders_are_reachable_and_activated_from_the_keyboard(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| view.open_settings(window, cx));
        window.render_frame(cx);
        window.click("settings-category-shortcuts", cx);
        window.render_frame(cx);
    })
    .unwrap();

    let mut focused = false;
    for _ in 0..64 {
        cx.update_window(window, |_, window, cx| {
            window.press("tab", cx);
        })
        .unwrap();
        cx.run_until_parked();
        focused = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.find("current-shortcut-record").focused() == Some(true)
            })
            .unwrap();
        if focused {
            break;
        }
    }
    assert!(focused, "the shortcut recorder must be in the Tab order");
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).shortcuts.recording,
            Some(ShortcutSlot::CurrentApp)
        );
    });
}

#[gpui_kit::test]
fn settings_add_action_shortcut_opens_a_picker_and_saves_the_recorded_scope(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.shortcuts.catalog = vec![ShortcutCatalogAction {
                action_id: "developer.terminal.open".to_owned(),
                title: "Open terminal".to_owned(),
                localized_titles: Default::default(),
                context_types: vec!["core.file".to_owned()],
            }];
            view.open_settings(window, cx);
        });
        window.render_frame(cx);
        window.click("settings-category-shortcuts", cx);
        window.render_frame(cx);
        assert!(window.try_find("settings-action-shortcut-search").is_none());
        window.click("settings-add-action-shortcut", cx);
        window.render_frame(cx);
        assert!(window.try_find("settings-action-picker").is_some());
        window.click("settings-action-choice-developer.terminal.open", cx);
        window.render_frame(cx);
        window.click("settings-action-picker-record", cx);
        view.update(cx, |view, cx| {
            view.handle_key(
                "k",
                KeyModifiers {
                    command: false,
                    control: false,
                    shift: false,
                    alt: true,
                    modified: true,
                },
                window,
                cx,
            );
        });
        window.render_frame(cx);
        window.click("settings-action-picker-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.settings_editor.action_picker_open);
        let binding = view
            .settings_editor
            .draft
            .action_shortcuts
            .iter()
            .find(|binding| binding.action_id == "developer.terminal.open")
            .expect("the recorded action shortcut is saved to the draft");
        assert_eq!(binding.shortcut, "alt+KeyK");
        assert!(!binding.global);
    });
}

#[gpui_kit::test]
fn settings_use_localized_engine_names_and_full_width_path_fields(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.locale = Locale::Zh;
            view.model.set_locale(Locale::Zh);
            view.settings_editor.draft.file_search_engine =
                anywhere_platform::FileSearchEngine::SystemIndex;
            view.settings_editor.model.selected = SettingsCategory::Search;
            view.screen = Screen::Settings;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        if cfg!(windows) {
            assert_eq!(
                window.find("cycle-file-engine").label(),
                Some(l10n::search_engine_windows_index(Locale::Zh).as_str())
            );
            assert_eq!(
                window.find("application-search-add-folders").label(),
                Some(l10n::windows_add_application_folders(Locale::Zh).as_str())
            );
            assert_eq!(
                window.find("application-search-add-shortcuts").label(),
                Some(l10n::windows_add_application_shortcuts(Locale::Zh).as_str())
            );
        } else {
            assert_eq!(
                window.find("cycle-file-engine").label(),
                Some(l10n::search_engine_system_index(Locale::Zh).as_str())
            );
        }

        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::ApplicationHistory;
            view.app_history.sources = vec![crate::host::AppHistorySource {
                id: "source".into(),
                title: "Source".into(),
                file_hint: "history.json".into(),
                action_enabled: true,
                path: None,
                mode: anywhere_core::features::application_history::model::SourceMode::Custom,
            }];
            view.app_history.selected_source = Some("source".into());
            cx.notify();
        });
        window.render_frame(cx);
        assert!(
            window
                .find("settings-field-history-custom-path")
                .bounds()
                .size
                .width
                > px(300.0)
        );
        assert!(window.find("history-custom-path").bounds().size.width > px(300.0));
        assert!(
            window
                .find("settings-field-app-history-custom-adapters-row")
                .bounds()
                .size
                .width
                > px(300.0)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_history_filter_matches_identity_and_enabled_state(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::ApplicationHistory;
            view.screen = Screen::Settings;
            view.app_history.sources = vec![
                crate::host::AppHistorySource {
                    id: "code".to_owned(),
                    title: "Code".to_owned(),
                    file_hint: "~/Library/Application Support/Code/state.vscdb".to_owned(),
                    action_enabled: true,
                    path: None,
                    mode:
                        anywhere_core::features::application_history::model::SourceMode::Automatic,
                },
                crate::host::AppHistorySource {
                    id: "browser".to_owned(),
                    title: "Browser".to_owned(),
                    file_hint: "~/Library/Application Support/Browser/History".to_owned(),
                    action_enabled: false,
                    path: None,
                    mode:
                        anywhere_core::features::application_history::model::SourceMode::Automatic,
                },
            ];
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("history-action-code").is_some());
        assert!(window.try_find("history-action-browser").is_some());

        window.click("history-filter-enabled", cx);
        window.render_frame(cx);
        assert!(window.try_find("history-action-code").is_some());
        assert!(window.try_find("history-action-browser").is_none());

        window.click("history-filter-all", cx);
        window.click("history-settings-filter", cx);
        window.input("browser", cx);
        window.render_frame(cx);
        assert!(window.try_find("history-action-browser").is_some());
        assert!(window.try_find("history-action-code").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_sources_load_without_starting_an_application_history_query(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::ApplicationHistory;
            view.screen = Screen::Settings;
            view.app_history.loading = false;
            view.apply_auxiliary_event(
                LiveEvent::AppHistorySources {
                    request: 1,
                    session_id: Some(7),
                    sources: vec![crate::host::AppHistorySource {
                        id: "code".to_owned(),
                        title: "Code".to_owned(),
                        file_hint: "~/Library/Application Support/Code/state.vscdb"
                            .to_owned(),
                        action_enabled: true,
                        path: None,
                        mode: anywhere_core::features::application_history::model::SourceMode::Automatic,
                    }],
                },
                cx,
            );
            assert_eq!(view.app_history.sources.len(), 1);
            assert!(!view.app_history.loading);
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("history-action-code").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn capture_and_excluded_apps_belong_to_their_settings_pages(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::Context;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("capture-clipboard").is_none());
        assert!(window.try_find("excluded-apps").is_none());
        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::Clipboard;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("capture-clipboard").is_some());
        view.update(cx, |view, cx| {
            view.settings_editor.model.selected = SettingsCategory::Search;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("excluded-apps").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn settings_switch_starts_the_new_category_at_the_top(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| view.open_settings(window, cx));
        window.render_frame(cx);
    })
    .unwrap();

    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.settings_editor.scrolls[SettingsCategory::Shortcuts as usize]
                .set_offset(gpui_kit::point(px(0.), px(-240.)));
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.click("settings-category-search", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(
            view.settings_editor.model.selected,
            SettingsCategory::Search
        );
        assert_eq!(
            view.settings_editor.scrolls[SettingsCategory::Search as usize].offset(),
            gpui_kit::point(px(0.), px(0.))
        );
    });
}

#[gpui_kit::test]
fn settings_escape_runs_before_a_focused_text_control(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.shortcuts.catalog = vec![ShortcutCatalogAction {
                action_id: "developer.terminal.open".to_owned(),
                title: "Open terminal".to_owned(),
                localized_titles: Default::default(),
                context_types: vec!["core.file".to_owned()],
            }];
            view.open_settings(window, cx);
        });
        window.render_frame(cx);
        window.click("settings-category-shortcuts", cx);
        window.render_frame(cx);
        window.click("settings-add-action-shortcut", cx);
        window.render_frame(cx);
        assert!(window.try_find("settings-action-picker").is_some());
        window.click("settings-action-shortcut-search", cx);
    })
    .unwrap();

    cx.update_window(window, |_, window, cx| {
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Settings);
        assert!(!view.settings_editor.action_picker_open);
    });
    cx.update_window(window, |_, window, cx| {
        window.click("done-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).screen, Screen::Launcher);
    });

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| view.open_settings(window, cx));
        window.render_frame(cx);
        window.click("settings-category-general", cx);
        window.click("open-at-login", cx);
        window.render_frame(cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Settings);
        assert!(view.settings_editor.close_pending);
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Settings);
        assert!(!view.settings_editor.close_pending);
    });
}

#[gpui_kit::test]
fn leaving_settings_restores_the_launcher_editor_focus(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| view.open_settings(window, cx));
        window.render_frame(cx);
        window.click("done-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).screen, Screen::Launcher);
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let focused = view.update(cx, |view, cx| view.active_editor_focused(window, cx));
        assert!(
            focused,
            "Done must restore the Launcher editor focus before returning"
        );
        window.press("x", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).input.read(cx).value().as_ref(), "x");
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| view.open_settings(window, cx));
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(view.read(cx).screen, Screen::Launcher);
        let focused = view.update(cx, |view, cx| view.active_editor_focused(window, cx));
        assert!(
            focused,
            "Escape must restore the Launcher editor focus before returning"
        );
        window.press("y", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).input.read(cx).value().as_ref(), "xy");
    });
}

#[gpui_kit::test]
fn launcher_hotkey_closes_clean_settings_and_keeps_dirty_settings_visible(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let shown = view.update(cx, |view, cx| {
            view.presence.presented = true;
            view.screen = Screen::Settings;
            view.activate_from_hotkey(InputMode::Search, window, cx)
        });
        assert!(shown);
        assert_eq!(view.read(cx).screen, Screen::Launcher);

        view.update(cx, |view, cx| {
            view.presence.presented = true;
            view.screen = Screen::Settings;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            assert!(!view.activate_from_hotkey(InputMode::Search, window, cx));
        });
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Settings);
        assert!(view.settings_editor.close_pending);
        assert!(view.notice.is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn web_packages_escape_returns_to_settings(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.open_screen(Screen::WebPackages, window, cx);
            assert_eq!(view.screen, Screen::WebPackages);
            assert!(view.is_settings_surface());
            assert!(!view.dismiss_on_focus_loss());
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::Settings);
            assert!(view.is_settings_surface());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn launcher_escape_restores_the_flutter_root_navigation_chain(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.presence.presented = true;
            view.input_mode = InputMode::Environment;
            view.model.set_query(String::new());
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.presence.presented);
            assert_eq!(view.input_mode, InputMode::Search);

            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.presence.presented);

            view.presence.presented = true;
            view.input_mode = InputMode::Content;
            view.model.set_query("captured".to_owned());
            set_textarea_value(&view.content_input, "captured", window, cx);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.presence.presented);
            assert_eq!(view.input_mode, InputMode::Environment);
            assert_eq!(view.model.query, "");
            assert_eq!(view.input.read(cx).value().as_ref(), "");
            assert_eq!(view.content_input.read(cx).value().as_ref(), "");

            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.presence.presented);
            assert_eq!(view.input_mode, InputMode::Search);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.presence.presented);

            view.presence.presented = true;
            view.input_mode = InputMode::Environment;
            view.model.set_query("git".to_owned());
            set_input_value(&view.input, "git", window, cx);
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.presence.presented);
            assert_eq!(view.input_mode, InputMode::Environment);
            assert_eq!(view.model.query, "");
            assert_eq!(view.input.read(cx).value().as_ref(), "");

            view.model.rows = vec![LauncherRow {
                id: SharedString::from("menu-item"),
                title: SharedString::from("Menu item"),
                subtitle: None,
                shortcut: None,
                icon: "menu".to_owned(),
                kind: CandidateKind::Action,
                enabled: true,
                is_menu: true,
                menu_path: Vec::new(),
                command_id: None,
                file_kind: None,
                source_label: None,
                icon_identity: None,
                execution_context_id: None,
                primary_action: false,
            }]
            .into();
            assert!(view.model.open_selected_menu());
            view.model.set_query("export".to_owned());
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert_eq!(view.model.query, "");
            assert_eq!(view.model.menu_path(), Some(&[][..]));
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(view.model.menu_path().is_none());
            assert!(view.presence.presented);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn returning_from_a_tool_restores_the_parent_workspace(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.model.set_query("git".to_owned());
            view.model.selected_scope = None;
            view.model.selected = 0;
            view.capture_parent_workspace(cx);
            view.screen = Screen::UnsupportedTool;
            view.model.set_query("tool query".to_owned());
            view.go_back(window, cx);
            assert_eq!(view.screen, Screen::Launcher);
            assert_eq!(view.model.query, "git");
            assert_eq!(
                view.model.selection_id(),
                Some("git-status"),
                "selection is restored by stable ID"
            );
        });
    })
    .unwrap();
}

#[test]
fn shortcut_recorder_normalizes_product_hotkeys() {
    assert_eq!(
        normalized_shortcut("space", false, true, true, false),
        Some("control+alt+Space".to_owned())
    );
    assert_eq!(
        normalized_shortcut("space", false, false, true, false),
        Some("alt+Space".to_owned())
    );
    assert_eq!(
        normalized_shortcut("k", false, false, true, false),
        Some("alt+KeyK".to_owned())
    );
    assert_eq!(
        normalized_shortcut("k", false, true, true, true),
        Some("control+alt+shift+KeyK".to_owned())
    );
    assert_eq!(
        normalized_shortcut("space", true, false, true, false),
        Some("alt+meta+Space".to_owned())
    );
    assert_eq!(normalized_shortcut("k", false, false, false, true), None);
}

#[gpui_kit::test]
fn option_modified_launcher_shortcuts_record_and_validate(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.shortcuts.recording = Some(ShortcutSlot::CurrentApp);
            view.handle_key(
                "space",
                KeyModifiers {
                    command: false,
                    control: true,
                    shift: false,
                    alt: true,
                    modified: true,
                },
                window,
                cx,
            );
            assert_eq!(
                view.shortcuts.current_input.read(cx).value().as_ref(),
                "control+alt+Space"
            );

            view.shortcuts.recording = Some(ShortcutSlot::Universal);
            view.handle_key(
                "space",
                KeyModifiers {
                    command: false,
                    control: false,
                    shift: false,
                    alt: true,
                    modified: true,
                },
                window,
                cx,
            );
            assert_eq!(
                view.shortcuts.universal_input.read(cx).value().as_ref(),
                "alt+Space"
            );
            assert!(view.preferences_from_editors(cx).validate().is_ok());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn context_shortcut_builder_keeps_routes_and_records_one_chord(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            let action = ShortcutCatalogAction {
                action_id: "developer.terminal.open".to_owned(),
                title: "Open terminal".to_owned(),
                localized_titles: Default::default(),
                context_types: vec!["core.file".to_owned()],
            };
            view.open_context_shortcut_draft(None, window, cx);
            set_input_value(&view.shortcuts.context_name_input, "Open here", window, cx);
            view.add_context_shortcut_action(&action, cx);
            view.shortcuts.recording = Some(ShortcutSlot::ContextShortcut);
            view.handle_key(
                "k",
                KeyModifiers {
                    command: false,
                    control: true,
                    shift: true,
                    alt: true,
                    modified: true,
                },
                window,
                cx,
            );
            assert_eq!(
                view.shortcuts
                    .context_draft
                    .as_ref()
                    .and_then(|draft| draft.shortcut.as_deref()),
                Some("control+alt+shift+KeyK")
            );
            view.commit_context_shortcut_draft(cx).unwrap();
            let binding = view
                .settings_editor
                .draft
                .action_shortcuts
                .iter()
                .find(|binding| binding.action_id.starts_with("intent:"))
                .unwrap();
            assert_eq!(binding.title, "Open here");
            assert_eq!(binding.routes.len(), 1);
            assert_eq!(binding.routes[0].context_type.as_deref(), Some("core.file"));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn native_destinations_open_typed_surfaces(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};

    let (window, view) = open_view(cx);
    let shelf_requested = Rc::new(Cell::new(false));
    let observed = shelf_requested.clone();
    let _shelf_event = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::OpenClipboardShelf {
                observed.set(true);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.open_action_destination(
                HostView::NativeTool {
                    id: "clipboard.shelf".to_owned(),
                    input: String::new(),
                },
                window,
                cx,
            );
        });
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Launcher);
        assert!(shelf_requested.get());
    });

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.go_back(window, cx);
            view.open_action_destination(
                HostView::NativeTool {
                    id: "unknown.tool".to_owned(),
                    input: "{\"value\":1}".to_owned(),
                },
                window,
                cx,
            );
        });
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::UnsupportedTool);
        assert_eq!(
            view.web_surface
                .unsupported_tool
                .as_ref()
                .map(|(id, _)| id.as_str()),
            Some("unknown.tool")
        );
    });
}

#[gpui_kit::test]
fn web_teardown_clears_every_document_owned_field(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.web_surface.document = Some(WebDocument {
                handle: "handle-a".to_owned(),
                origin: "anywhere-plugin://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.plugin.local".to_owned(),
                entry: "index.html".to_owned(),
                title: "Tool".to_owned(),
                builtin: false,
            });
            view.web_surface.ready = true;
            view.web_surface.error = Some("old error".to_owned());
            view.web_surface.invocation_id = Some("invocation".to_owned());
            view.web_surface.actions_open = true;
            view.web_surface.action_selected = 1;
            view.web_surface.state_handle = Some("state".to_owned());
            view.web_surface.state_version = 9;
            assert!(event_handle_matches(view, "handle-a"));
            assert!(!event_handle_matches(view, "handle-b"));
            view.close_web_surface(cx);
            assert!(view.web_surface.document.is_none());
            assert!(!view.web_surface.ready);
            assert!(view.web_surface.error.is_none());
            assert!(view.web_surface.invocation_id.is_none());
            assert!(!view.web_surface.actions_open);
            assert_eq!(view.web_surface.action_selected, 0);
            assert!(view.web_surface.state_handle.is_none());
            assert_eq!(view.web_surface.state_version, 0);
            assert!(!event_handle_matches(view, "handle-a"));
        });
    });
}

#[gpui_kit::test]
fn web_package_review_actions_stay_inside_the_window(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, _window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebPackages;
            view.web_surface.review = Some(crate::host::WebPackageReview {
                token: "review-token".into(),
                id: "dev.example.text-pad".into(),
                name: "Text Pad".into(),
                version: "1.1.1".into(),
                digest: "0123456789abcdef".into(),
                permissions: vec!["Read Launcher search text".into()],
                updating: false,
            });
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let approve = window.find("approve-web-package").bounds();
        let viewport = window.viewport_size();
        assert!(approve.left() >= px(0.));
        assert!(approve.top() >= px(0.));
        assert!(approve.bottom() <= viewport.height);
        assert!(
            approve.right() <= viewport.width,
            "approve={approve:?} viewport={viewport:?}"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn task_results_open_tasks_before_dismissing_launcher(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};

    let (_window, view) = open_view(cx);
    let opened = Rc::new(Cell::new(false));
    let observed = opened.clone();
    let _event = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if matches!(event, LauncherEvent::OpenTasks(task_id) if task_id == "task") {
                observed.set(true);
            }
        })
    });
    let task_result = || anywhere_protocol::ActionResult {
        session_id: 1,
        revision: 1,
        status: anywhere_protocol::ActionStatus::Succeeded,
        error: None,
        warning: None,
        output_text: None,
        view: None,
        close_window: true,
        execution_session_id: Some("task".to_owned()),
    };
    let (generation_before, dismissed, generation_after) = cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.presence.presented = true;
            let generation = view.live_state.generation;
            let dismissed = view.finish_action_result(&task_result(), false, cx);
            let generation_after = view.live_state.generation;
            (generation, dismissed, generation_after)
        })
    });
    assert!(dismissed);
    assert_eq!(generation_after, generation_before.wrapping_add(1));
    assert!(opened.get());

    opened.set(false);
    let (generation_before, dismissed, still_presented, generation_after_warning) =
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.presence.presented = true;
                let generation = view.live_state.generation;
                let mut warning = task_result();
                warning.warning = Some("usage unavailable".to_owned());
                let dismissed = view.finish_action_result(&warning, false, cx);
                (
                    generation,
                    dismissed,
                    view.presence.presented,
                    view.live_state.generation,
                )
            })
        });
    assert!(!dismissed);
    assert!(still_presented);
    assert_eq!(generation_after_warning, generation_before);
    assert!(!opened.get());
}

#[gpui_kit::test]
fn selecting_a_started_task_applies_to_the_next_tasks_snapshot(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let row = |id: &str| crate::host::ExecutionRow {
        id: id.to_owned(),
        title: id.to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.tasks.set_filter("old-task".to_owned());
            view.select_execution("new-task".to_owned(), cx);
            view.apply_tasks_rows(vec![row("old-task"), row("new-task")], cx);
            assert!(view.tasks.filter.is_empty());
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("new-task")
            );
            assert!(view.tasks.pending_selection.is_none());
            assert!(view.tasks.detail.is_none());

            view.select_execution("old-task".to_owned(), cx);
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("old-task")
            );
        });
    });
}

#[gpui_kit::test]
fn task_navigation_at_a_boundary_keeps_the_current_detail(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = |id: &str| crate::host::ExecutionRow {
        id: id.to_owned(),
        title: id.to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "true".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.tasks.apply_rows(vec![row("first"), row("last")]);
            view.tasks.selected = 1;
            view.tasks.detail = Some(crate::host::ExecutionDetail {
                row: row("last"),
                stdout: "ready".into(),
                stderr: "".into(),
            });
            view.tasks.detail_request = 7;

            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.tasks.selected, 1);
            assert!(view.tasks.detail.is_some());
            assert_eq!(view.tasks.detail_request, 7);

            view.handle_key("up", KeyModifiers::default(), window, cx);
            assert_eq!(view.tasks.selected, 0);
            assert!(view.tasks.detail.is_none());
            assert_eq!(view.tasks.detail_request, 8);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn web_tool_actions_expose_reload_and_back(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.web_surface.invocation_id = Some("invocation".to_owned());
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("web-tool-actions").is_some());
        window.click("web-tool-actions", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("web-tool-actions-panel").is_some());
        view.update(cx, |view, cx| {
            assert!(window.try_find("web-tool-action-0").is_some());
            assert!(window.try_find("web-tool-action-1").is_some());
            view.web_surface.action_selected = 0;
            view.handle_key("enter", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::WebTool);
            assert!(!view.web_surface.actions_open);
            assert_eq!(
                view.web_surface.invocation_id.as_deref(),
                Some("invocation")
            );

            view.toggle_web_actions(window, cx);
            view.handle_key("down", KeyModifiers::default(), window, cx);
            assert_eq!(view.web_surface.action_selected, 1);
            view.handle_key("enter", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::Launcher);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn platform_escape_leaves_web_tool_only(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.notice = None;
            assert!(view.handle_web_view_escape_from_platform(window, cx));
            assert_eq!(view.screen, Screen::Launcher);
            assert!(!view.handle_web_view_escape_from_platform(window, cx));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn preview_completion_restores_the_editor_for_the_current_screen(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            assert!(view
                .active_editor_input()
                .is_some_and(|input| input == &view.input));
            view.screen = Screen::FileSearch;
            assert!(view
                .active_editor_input()
                .is_some_and(|input| input == &view.input));
            view.screen = Screen::ClipboardShelf;
            assert!(view
                .active_editor_input()
                .is_some_and(|input| input == &view.clipboard_input));
            view.screen = Screen::Tasks;
            assert!(view.active_editor_input().is_none());
        });
    });
}

#[gpui_kit::test]
fn web_settings_subpage_uses_the_shared_dirty_close_prompt(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebPackages;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("product-window-resize").is_none());
        if crate::platform::WINDOW_CONTROLS
            == crate::platform::WindowControlsStyle::TrailingCaptionButtons
        {
            assert!(window.try_find("product-window-minimize").is_some());
            window.click("product-window-close", cx);
        } else {
            assert!(window.try_find("product-window-minimize").is_none());
            assert!(window.try_find("product-window-close").is_none());
            view.update(cx, |view, cx| {
                assert!(!view.request_main_window_close(window, cx));
            });
        }
        window.render_frame(cx);
        assert!(window.try_find("settings-close-prompt").is_some());
    })
    .unwrap();
    dispatch_shortcut(cx, window, "escape");
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::WebPackages);
        assert!(!view.settings_editor.close_pending);
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        if crate::platform::WINDOW_CONTROLS
            == crate::platform::WindowControlsStyle::TrailingCaptionButtons
        {
            window.click("product-window-close", cx);
        } else {
            view.update(cx, |view, cx| {
                assert!(!view.request_main_window_close(window, cx));
            });
        }
        window.render_frame(cx);
        window.click("save-settings-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::Launcher);
        assert!(!view.presence.presented);
    });
}

#[gpui_kit::test]
fn settings_close_prompt_blocks_pointer_and_traps_focus(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::General;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.settings_editor.focus_handle.focus(window, cx);
            assert!(!view.request_settings_exit(SettingsExitIntent::Close, window, cx));
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx
        .update_window(window, |_, window, cx| {
            view.read(cx)
                .settings_editor
                .keep_editing_focus
                .is_focused(window)
        })
        .unwrap());

    let before = cx.update(|cx| view.read(cx).settings_editor.draft.launch_at_login);
    let switch_center = cx
        .update_window(window, |_, window, _| {
            window.find("open-at-login").bounds().center()
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.simulate_click(switch_center, gpui_kit::Modifiers::default());
    visual.run_until_parked();
    assert_eq!(
        visual.update(|_, cx| view.read(cx).settings_editor.draft.launch_at_login),
        before,
        "the scrim must block pointer interaction with the underlying switch"
    );

    visual.update(|window, cx| window.press("tab", cx));
    visual.run_until_parked();
    visual.update(|window, cx| {
        let view = view.read(cx);
        assert!(
            view.settings_editor.discard_focus.is_focused(window)
                || view.settings_editor.save_focus.is_focused(window)
        );
        assert!(!view.settings_editor.focus_handle.is_focused(window));
    });

    visual.update(|window, cx| window.press("x", cx));
    visual.run_until_parked();
    visual.update(|_, cx| {
        let view = view.read(cx);
        assert!(view.settings_editor.close_pending);
        assert_eq!(view.settings_editor.draft.launch_at_login, before);
    });

    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.settings_editor.keep_editing_focus.focus(window, cx);
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.press("space", cx));
    visual.run_until_parked();
    visual.update(|window, cx| {
        let view = view.read(cx);
        assert!(!view.settings_editor.close_pending);
        assert_eq!(view.screen, Screen::Settings);
        assert_eq!(view.settings_editor.draft.launch_at_login, before);
        assert!(view.settings_editor.focus_handle.is_focused(window));
    });
}

#[gpui_kit::test]
fn platform_escape_uses_the_settings_modal_route_and_stops_shortcut_recording(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::General;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            view.shortcuts.recording = Some(ShortcutSlot::CurrentApp);
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.settings_editor.focus_handle.focus(window, cx);
            assert!(!view.request_settings_exit(SettingsExitIntent::Close, window, cx));
            assert!(view.settings_editor.close_pending);
            assert_eq!(view.shortcuts.recording, None);
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();

    let original = cx.update(|cx| {
        view.read(cx)
            .shortcuts
            .current_input
            .read(cx)
            .value()
            .to_string()
    });
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            // The modal must remain authoritative even if a stale recorder
            // state is restored by an asynchronous path.
            view.shortcuts.recording = Some(ShortcutSlot::CurrentApp);
            cx.notify();
        });
    });
    cx.simulate_keystrokes(window, "ctrl-j");
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.settings_editor.close_pending);
        assert_eq!(
            view.shortcuts.current_input.read(cx).value(),
            original.as_str()
        );
    });

    let handled = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.handle_settings_escape_from_platform(window, cx)
            })
        })
        .unwrap();
    assert!(handled);
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(!view.settings_editor.close_pending);
        assert_eq!(view.screen, Screen::Settings);
        assert!(view.settings_editor.focus_handle.is_focused(window));
        assert_eq!(view.shortcuts.recording, None);
        assert_eq!(
            view.settings_editor.draft.launch_at_login,
            !view.preferences.launch_at_login
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn platform_escape_cancels_a_clipboard_confirmation_before_leaving_settings(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::Clipboard;
            view.settings_editor.modal_return_focus =
                Some(view.settings_editor.focus_handle.clone());
            view.clipboard_confirmation = Some(ClipboardConfirmation::Clear);
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);

    let handled = cx
        .update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.settings_editor.focus_handle.focus(window, cx);
                view.handle_settings_escape_from_platform(window, cx)
            })
        })
        .unwrap();
    assert!(handled);
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(view.clipboard_confirmation.is_none());
        assert_eq!(view.screen, Screen::Settings);
        assert!(view.settings_editor.focus_handle.is_focused(window));
    })
    .unwrap();
}

#[gpui_kit::test]
fn launcher_notice_uses_bounded_height_and_never_overlaps_rows(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.set_query("report".to_owned());
            view.model.rows = vec![file_row("one", "Report.pdf")].into();
            view.set_notice(
                "A long error detail\n".repeat(32),
                components::NoticeKind::Error,
            );
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("launcher-search-header").is_some());
        assert!(window.try_find("launcher-notice").is_some());
        assert!(window.try_find("candidate-0").is_some());
        assert!(window.try_find("launcher-notice-scroll").is_some());
    });
    let header = visual.update(|window, _| window.find("launcher-search-header").bounds());
    let notice = visual.update(|window, _| window.find("launcher-notice").bounds());
    let result = visual.update(|window, _| window.find("candidate-0").bounds());
    assert_eq!(notice.size.height, px(metrics::NOTICE_MAX_HEIGHT));
    assert!(notice.top() >= header.bottom());
    assert!(result.top() >= notice.bottom());
}

#[gpui_kit::test]
fn notice_changes_converge_to_the_latest_window_size(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.set_query("action".to_owned());
            let mut row = file_row("disabled", "Unavailable action");
            row.enabled = false;
            view.model.rows = vec![row].into();
            view.model.selected = 0;
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.activate_selection(cx);
            assert_eq!(
                view.desired_window_size().height,
                px(180.5),
                "the one-row failure state must include the notice"
            );
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    let viewport = cx
        .update_window(window, |_, window, _| window.viewport_size())
        .unwrap();
    assert_eq!(viewport.height, px(180.5));
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("candidate-0").is_some());
    });
    let result = visual.update(|window, _| window.find("candidate-0").bounds());
    assert!(result.bottom() <= viewport.height);
}

#[gpui_kit::test]
fn scope_bar_centers_short_groups_and_keeps_overflow_reachable(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.scopes = vec![
                crate::launcher::LauncherScope {
                    id: "one".to_owned(),
                    label: "One".to_owned(),
                    is_current_app: false,
                },
                crate::launcher::LauncherScope {
                    id: "two".to_owned(),
                    label: "Two".to_owned(),
                    is_current_app: false,
                },
            ];
            view.model.set_query("x".to_owned());
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("scope-one").is_some());
        assert!(window.try_find("scope-two").is_some());
    });
    let one = visual.update(|window, _| window.find("scope-one").bounds());
    let two = visual.update(|window, _| window.find("scope-two").bounds());
    let group_center = (one.left() + two.right()) / 2.0;
    let viewport_center = visual.update(|window, _| window.viewport_size().width) / 2.0;
    assert!((group_center - viewport_center).abs() <= px(2.));
}

#[gpui_kit::test]
fn scope_bar_overflow_starts_at_the_leading_edge_and_reveals_a_late_selection(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.scopes = (0..8)
                .map(|index| crate::launcher::LauncherScope {
                    id: format!("scope-{index}"),
                    label: format!("A very long scope label {index}"),
                    is_current_app: false,
                })
                .collect();
            view.model.set_query("x".to_owned());
            view.model.selected_scope = Some("scope-7".to_owned());
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let maximum = view.read(cx).scrolls.scope.max_offset().x;
        assert!(
            maximum > px(0.),
            "an overflowing scope group must expose horizontal scrolling"
        );
        let first = window.find("scope-scope-0").bounds();
        assert!(first.left() >= px(0.));
        assert!(
            first.left() < window.viewport_size().width,
            "the first scope must remain at the reachable leading edge"
        );
        view.read(cx).reveal_selected_scope();
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let last = window.find("scope-scope-7").bounds();
        let offset = view.read(cx).scrolls.scope.offset().x;
        assert!(
            last.left() >= px(0.) && last.right() <= window.viewport_size().width,
            "the selected late scope must be revealed inside the viewport: \
             last={last:?}, offset={offset:?}, viewport={:?}",
            window.viewport_size()
        );
        assert!(offset < px(0.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn shelf_header_respects_window_controls_and_escape_shares_keyboard_route(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.clipboard.query = "fixture".to_owned();
            cx.notify();
        });
    });
    sync_test_viewport(cx, window, gpui_kit::size(px(360.), px(360.)));
    let mut visual = VisualTestContext::from_window(window, cx);
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("shelf-state-icon").is_some());
        if crate::platform::WINDOW_CONTROLS
            == crate::platform::WindowControlsStyle::TrailingCaptionButtons
        {
            let minimize = window.find("product-window-minimize").bounds();
            let resize = window.find("product-window-resize").bounds();
            let close = window.find("product-window-close").bounds();
            assert!(minimize.right() <= resize.left());
            assert!(resize.right() <= close.left());
            assert!(close.left() > window.viewport_size().width / 2.0);
        } else {
            assert!(window.try_find("product-window-close").is_none());
            assert!(window.try_find("product-window-minimize").is_none());
            assert!(window.try_find("product-window-resize").is_none());
        }
    });
    let icon = visual.update(|window, _| window.find("shelf-state-icon").bounds());
    assert!(icon.top() >= px(metrics::TITLE_BAR_INSET));
    visual.update(|window, cx| {
        window.click("shelf-escape", cx);
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::ClipboardShelf);
        assert!(view.clipboard.query.is_empty());
    });

    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.clipboard.actions_open = true;
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("shelf-escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.clipboard.actions_open);
        assert_eq!(view.screen, Screen::ClipboardShelf);
    });

    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.clipboard.delete_pending = Some(42);
            cx.notify();
        });
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("shelf-escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.clipboard.delete_pending.is_none());
        assert_eq!(view.screen, Screen::ClipboardShelf);
    });
}

#[gpui_kit::test]
fn unsupported_tool_content_scrolls_to_long_input(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::UnsupportedTool;
            view.web_surface.unsupported_tool = Some((
                "tool".to_owned(),
                (0..60)
                    .map(|i| format!("line {i}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    let mut visual = VisualTestContext::from_window(window, cx);
    let maximum = visual.update(|_, cx| view.read(cx).scrolls.unsupported_tool.max_offset().y);
    assert!(maximum > px(0.));
    visual.update(|_, cx| {
        view.read(cx)
            .scrolls
            .unsupported_tool
            .set_offset(gpui_kit::point(px(0.), -maximum));
    });
    visual.update(|window, cx| window.render_frame(cx));
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("unsupported-tool-input-text").is_some());
    });
    let input = visual.update(|window, _| window.find("unsupported-tool-input-text").bounds());
    let viewport_height = visual.update(|window, _| window.viewport_size().height);
    assert!(input.bottom() <= viewport_height);
}

#[gpui_kit::test]
fn discarding_language_draft_restores_the_saved_locale(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::General;
            view.preferences.ui_language = UiLanguage::Zh;
            view.settings_editor.draft.ui_language = UiLanguage::Zh;
            view.locale = Locale::Zh;
            view.model.set_locale(Locale::Zh);
            view.settings_editor.draft.ui_language = UiLanguage::En;
            view.preferences.ui_theme = UiTheme::Dark;
            view.settings_editor.draft.ui_theme = UiTheme::Light;
            view.locale = Locale::En;
            view.model.set_locale(Locale::En);
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| view.discard_settings(window, cx));
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.locale, Locale::Zh);
        assert_eq!(view.settings_editor.draft.ui_language, UiLanguage::Zh);
        assert_eq!(view.settings_editor.draft.ui_theme, UiTheme::Dark);
        assert_eq!(view.screen, Screen::Launcher);
    });
}

#[gpui_kit::test]
fn clipboard_settings_expose_real_choices_and_confirm_destructive_changes(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.selected = SettingsCategory::Clipboard;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: true,
                recording_enabled: true,
                available: true,
                paused_until_ms: None,
                retention_days: 30,
                storage_usage: Some((0, 0)),
                notice: None,
                items: Vec::new(),
            });
            cx.notify();
        });
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("clipboard-retention", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("clipboard-retention-90").is_some());
        window.click("clipboard-retention-1", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).clipboard_confirmation,
            Some(ClipboardConfirmation::SetRetention(1))
        );
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("clipboard-confirm-prompt").is_some());
        window.click("clipboard-confirm-cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.clipboard_confirmation.is_none());
        assert!(!view.clipboard.loading);
    });

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("clipboard-pause", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "clipboard-resume",
            "clipboard-pause-5",
            "clipboard-pause-15",
            "clipboard-pause-60",
        ] {
            assert!(window.try_find(id).is_some(), "missing pause choice {id}");
        }
        window.click("clipboard-resume", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.settings_editor.clipboard_choice.is_none());
        assert!(view.clipboard_confirmation.is_none());
    });
}

#[gpui_kit::test]
fn closed_clipboard_status_channel_clears_loading_and_reports_an_error(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    drop(sender);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.clipboard.query_generation = 7;
            view.clipboard.loading = true;
            view.poll_clipboard_search(receiver, 7, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.clipboard.loading);
        assert_eq!(view.notice_kind, components::NoticeKind::Error);
        assert_eq!(
            view.notice.as_deref(),
            Some(l10n::clipboard_settings_failed(Locale::En).as_str())
        );
    });
}

#[gpui_kit::test]
fn task_notices_keep_success_and_help_semantics(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let row = crate::host::ExecutionRow {
        id: "task".to_owned(),
        title: "npm Run · task".to_owned(),
        directory: "/tmp/project".to_owned(),
        command: "npm run task".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
        output_revision: 1,
        exit_code: Some(0),
        started_at_ms: 0,
        finished_at_ms: Some(1),
        output_truncated: false,
        error: None,
    };
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(vec![row.clone()], cx);
            view.tasks.detail = Some(crate::host::ExecutionDetail {
                row,
                stdout: "ready".into(),
                stderr: "".into(),
            });
        });
    })
    .unwrap();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.execute_task_action(0, cx);
            assert_eq!(view.notice_kind, components::NoticeKind::Success);
            view.set_notice(
                l10n::task_keyboard_help(Locale::En, "⌘F", "⌘."),
                components::NoticeKind::Info,
            );
            assert_eq!(view.notice_kind, components::NoticeKind::Info);
        });
    });
}

#[gpui_kit::test]
fn key_dispatch_precedence_preserves_recording_overlays_shortcuts_and_preview(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);

    // 1. Recording shortcut captures Escape and invalid keys without leaving Settings.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.shortcuts.recording = Some(ShortcutSlot::CurrentApp);
            view.handle_key("k", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::Settings);
            assert_eq!(view.shortcuts.recording, Some(ShortcutSlot::CurrentApp));
            assert_eq!(view.notice_kind, components::NoticeKind::Warning);

            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert_eq!(view.screen, Screen::Settings);
            assert_eq!(view.shortcuts.recording, None);
        });
    })
    .unwrap();

    // 2. Clipboard action overlay intercepts Cmd+K and falls through after closing on other keys.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.surface_focus_handle.focus(window, cx);
            view.clipboard.actions_open = true;
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, _| {
            assert!(!view.clipboard.actions_open);
            assert_eq!(view.screen, Screen::Clipboard);
            view.clipboard.actions_open = true;
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(!view.clipboard.actions_open);
        assert!(view
            .clipboard_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
    })
    .unwrap();

    // 3. Preview Escape closes preview only, keeping current screen intact.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.preview.open = true;
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.preview.open);
            assert_eq!(view.screen, Screen::FileSearch);
        });
    })
    .unwrap();

    // 4. User local shortcut takes precedence over built-in Cmd+F on Clipboard.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.surface_focus_handle.focus(window, cx);
            view.notice = Some("existing".into());
            view.preferences.action_shortcuts = vec![ActionShortcutBinding {
                action_id: "user.custom".to_owned(),
                title: "Custom".to_owned(),
                shortcut: if cfg!(target_os = "macos") {
                    "meta+KeyF"
                } else {
                    "control+KeyF"
                }
                .to_owned(),
                global: false,
                routes: Vec::new(),
            }];
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-f");
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.notice.is_none());
            assert!(!view
                .clipboard_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
            view.preferences.action_shortcuts.clear();
        });
    })
    .unwrap();

    // 5. Root navigation intercepts Escape before Launcher root escape runs.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Launcher;
            view.root_navigation_open = true;
            view.handle_key("escape", KeyModifiers::default(), window, cx);
            assert!(!view.root_navigation_open);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn gpui_keymap_dispatches_fixed_commands_while_input_is_focused_without_swallowing_typing(
    cx: &mut TestAppContext,
) {
    use std::{cell::Cell, rc::Rc};

    let (window, view) = open_view(cx);

    // 1. Focused Launcher input keeps plain typing and dispatches Cmd+Enter / Cmd+, / Cmd+Q.
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view
            .read(cx)
            .input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window));
    })
    .unwrap();
    cx.simulate_input(window, "json");
    cx.update(|cx| {
        assert_eq!(view.read(cx).input.read(cx).value().as_ref(), "json");
    });

    dispatch_shortcut(cx, window, "cmd-enter");
    cx.update(|cx| {
        assert!(view.read(cx).root_navigation_open);
    });
    dispatch_shortcut(cx, window, "cmd-enter");
    cx.update(|cx| {
        assert!(!view.read(cx).root_navigation_open);
    });

    dispatch_shortcut(cx, window, "cmd-,");
    cx.update(|cx| {
        assert_eq!(view.read(cx).screen, Screen::Settings);
    });

    // 2. Settings dispatches Cmd+S (SaveSettings) and Cmd+Q (QuitApp).
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.settings_editor.draft.launch_at_login = true;
            view.settings_editor.focus_handle.focus(window, cx);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-s");
    cx.update(|cx| {
        assert!(view.read(cx).preferences.launch_at_login);
    });

    let quit_requested = Rc::new(Cell::new(false));
    let observed_quit = quit_requested.clone();
    let _quit_sub = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::QuitRequested {
                observed_quit.set(true);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Launcher;
            view.input.update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-q");
    assert!(quit_requested.get());

    // 3. ClipboardShelf with focused input dispatches Cmd+K, Cmd+Shift+Backspace, and plain typing.
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: false,
                recording_enabled: false,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1, 4)),
                notice: None,
                items: vec![crate::host::ClipboardItem {
                    id: 42,
                    summary: "alpha".into(),
                    kind: "text".into(),
                    copied_at_ms: 0,
                    byte_count: 5,
                }],
            });
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.clipboard_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();
    cx.simulate_input(window, "hi");
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).clipboard_input.read(cx).value().as_ref(),
            "hi"
        );
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.handle_key("down", KeyModifiers::default(), window, cx);
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update(|cx| {
        assert!(view.read(cx).clipboard.actions_open);
    });
    dispatch_shortcut(cx, window, "cmd-k");
    cx.update(|cx| {
        assert!(!view.read(cx).clipboard.actions_open);
    });
    dispatch_shortcut(cx, window, "cmd-shift-backspace");
    cx.update(|cx| {
        assert_eq!(view.read(cx).clipboard.delete_pending, Some(42));
    });

    // 4. Tasks dispatches Cmd+L (RestoreLauncher) even while filter input is focused.
    let restored = Rc::new(Cell::new(false));
    let observed_restore = restored.clone();
    let _restore_sub = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::RestoreLauncher {
                observed_restore.set(true);
            }
        })
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(
                vec![crate::host::ExecutionRow {
                    id: "t1".into(),
                    title: "Task".into(),
                    directory: "/tmp".into(),
                    command: "echo".into(),
                    status: anywhere_protocol::ExecutionSessionStatus::Running,
                    output_revision: 0,
                    exit_code: None,
                    started_at_ms: 0,
                    finished_at_ms: None,
                    output_truncated: false,
                    error: None,
                }],
                cx,
            );
            view.tasks_filter_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "cmd-l");
    assert!(restored.get());
}
