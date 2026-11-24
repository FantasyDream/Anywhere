use super::*;
use crate::host::ActionShortcutBinding;
use std::{cell::Cell, rc::Rc};

#[gpui_kit::test]
fn clicking_settings_again_reactivates_an_existing_settings_surface(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        assert!(view.update(cx, |view, cx| view.open_settings(window, cx)));
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            view.input.update(cx, |input, cx| input.focus(window, cx));
            assert!(view.input.read(cx).focus_handle(cx).is_focused(window));
            assert!(view.open_settings(window, cx));
        });
        window.render_frame(cx);
        view.update(cx, |view, _| {
            assert_eq!(view.screen, Screen::Settings);
            assert!(view.presence.presented);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn clipboard_shelf_shortcut_opens_its_tool_while_settings_is_open(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
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
    let requested = Rc::new(Cell::new(false));
    let observed = requested.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
            if event == &LauncherEvent::OpenClipboardShelf {
                observed.set(true);
            }
        })
    });

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.open_settings(window, cx));
            view.live = Some(live);
            view.runtime_state = RuntimeState::Ready;
            let opened = view.activate_action_shortcut(
                ActionShortcutBinding {
                    action_id: anywhere_core::features::clipboard::SHELF_ACTION_ID.to_owned(),
                    title: "Clipboard Shelf".to_owned(),
                    shortcut: "control+KeyV".to_owned(),
                    global: true,
                    routes: Vec::new(),
                },
                window,
                cx,
            );
            assert!(!opened, "the launcher must stay out of the shelf path");
            assert_eq!(view.screen, Screen::Settings);
        });
    })
    .unwrap();
    assert!(requested.get());
}

#[gpui_kit::test]
fn recognized_text_switches_to_the_content_editor(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let selected = "https://example.org/selected";
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.begin_activation_prefill(true, cx);
            view.apply_activation_prefill(selected, window, cx);
            assert_eq!(view.input_mode, InputMode::Content);
            assert_eq!(view.model.query, selected);
            assert_eq!(view.content_input.read(cx).value().as_ref(), selected);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigation_cancels_pending_activation_prefill(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.begin_activation_prefill(true, cx);
            view.mark_scope_user_controlled();
            assert!(!view.activation_prefill.enabled);
        });
    });
}

#[gpui_kit::test]
fn content_editor_arrows_still_navigate_result_rows(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Content;
            view.model.set_query("captured".to_owned());
            view.model.rows = vec![
                file_row("first", "first.txt"),
                file_row("second", "second.txt"),
            ]
            .into();
            view.model.selected = 0;
            view.content_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();

    dispatch_shortcut(cx, window, "down");

    cx.update(|cx| {
        assert_eq!(view.read(cx).model.selected, 1);
    });
}

#[gpui_kit::test]
fn content_editor_enter_activates_selection_without_inserting_a_newline(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Content;
            view.model.set_query("captured".to_owned());
            view.model.rows = vec![file_row("first", "first.txt")].into();
            view.model.selected = 0;
            set_textarea_value(&view.content_input, "captured", window, cx);
            view.content_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();

    dispatch_shortcut(cx, window, "enter");

    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.content_input.read(cx).value().as_ref(), "captured");
        assert!(
            view.notice.is_some(),
            "Enter should activate the selected action"
        );
    });
}

#[gpui_kit::test]
fn content_editor_shift_enter_still_inserts_a_newline(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Content;
            set_textarea_value(&view.content_input, "captured", window, cx);
            view.content_input
                .update(cx, |input, cx| input.focus(window, cx));
        });
    })
    .unwrap();

    dispatch_shortcut(cx, window, "shift-enter");

    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.content_input.read(cx).value().as_ref(), "\ncaptured");
        assert!(view.notice.is_none());
    });
}
