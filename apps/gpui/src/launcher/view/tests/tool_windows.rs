use super::*;
use crate::host::LiveEvent;
use anywhere_protocol::{HostView, InputMode};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn write_test_awplugin(path: &std::path::Path, id: &str, name: &str, command_title: &str) {
    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "id": id,
        "name": name,
        "version": "1.0.0",
        "apiVersion": "1",
        "platforms": ["macos"],
        "entry": "index.html",
        "permissions": [],
        "commands": [{
            "id": "open",
            "title": command_title,
            "keywords": ["third", "party", "custom"],
            "accepts": [],
            "entrypoint": "workspace"
        }]
    });
    let entries: [(&str, Vec<u8>); 2] = [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("index.html", b"<!doctype html><p>third-party</p>".to_vec()),
    ];
    let mut archive = Vec::new();
    let mut central = Vec::new();
    for (entry_name, data) in &entries {
        let name_bytes = entry_name.as_bytes();
        let sum = crc32(data);
        let len = u32::try_from(data.len()).unwrap();
        let name_len = u16::try_from(name_bytes.len()).unwrap();
        let offset = u32::try_from(archive.len()).unwrap();

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&sum.to_le_bytes());
        archive.extend_from_slice(&len.to_le_bytes());
        archive.extend_from_slice(&len.to_le_bytes());
        archive.extend_from_slice(&name_len.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name_bytes);
        archive.extend_from_slice(data);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&((3u16 << 8) | 20).to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&sum.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&(0o100644u32 << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = u32::try_from(archive.len()).unwrap();
    let cd_size = u32::try_from(central.len()).unwrap();
    let count = u16::try_from(entries.len()).unwrap();
    archive.extend_from_slice(&central);
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&cd_size.to_le_bytes());
    archive.extend_from_slice(&cd_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    std::fs::write(path, archive).unwrap();
}

fn open_live_web_tool(
    live: &LiveLauncher,
    query: &str,
    command_id: &str,
) -> (
    anywhere_protocol::SessionId,
    String,
    crate::host::WebDocument,
) {
    let id = live.reserve_activation().unwrap();
    let LiveEvent::SharedState(initial) = live
        .activate_session(id, InputMode::Search)
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected activation state");
    };
    let LiveEvent::SharedState(searched) = live
        .update_input(
            query.to_owned(),
            initial.revision + 1,
            None,
            InputMode::Search,
        )
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected search state");
    };
    let candidate = searched
        .candidates
        .iter()
        .find(|candidate| candidate.command_id.as_deref() == Some(command_id))
        .unwrap_or_else(|| panic!("missing command candidate {command_id}"));
    let LiveEvent::ActionResult(result) = live
        .execute(searched.revision, candidate.id.clone())
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected action execution result");
    };
    let Some(HostView::WebPlugin { invocation_id }) = result.view else {
        panic!("expected HostView::WebPlugin");
    };
    let LiveEvent::WebDocument(document) = live
        .open_web_document(invocation_id.clone(), None)
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected opened WebDocument");
    };
    (id, invocation_id, document)
}

#[gpui_kit::test]
fn builtin_json_workspace_stages_automatically_and_hides_removal_actions(cx: &mut TestAppContext) {
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
    let LiveEvent::WebPackages(packages) = live.web_packages().blocking_recv().unwrap() else {
        panic!("expected WebPackages");
    };
    let json_pkg = packages
        .iter()
        .find(|package| package.id == "dev.anywhere.json-workspace")
        .expect("built-in JSON Workspace should load without manual import");
    assert!(json_pkg.builtin);
    assert!(json_pkg.enabled);
    assert!(json_pkg.available);
    assert!(!json_pkg.rollback);

    let (window, view) = open_view(cx);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebPackages;
            view.presence.presented = true;
            view.web.apply(packages);
            view.web.selected = 0;
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("web-package-0").is_some());
        assert!(window
            .try_find("web-toggle-dev.anywhere.json-workspace")
            .is_some());
        assert!(window.try_find("web-rollback").is_none());
        assert!(window.try_find("web-uninstall").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn standalone_tool_window_is_unavailable_for_non_web_screens_and_available_for_builtin_json_workspace(
    cx: &mut TestAppContext,
) {
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
    let (session_id, invocation_id, builtin_doc) = open_live_web_tool(
        &live,
        "JSON Workspace",
        "web:dev.anywhere.json-workspace:open",
    );
    assert!(builtin_doc.builtin);

    let (window, view) = open_view(cx);
    for non_web_screen in [
        Screen::FileSearch,
        Screen::Clipboard,
        Screen::ApplicationHistory,
    ] {
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.live = Some(live.clone());
                view.presence.presented = true;
                view.live_state.session_id = Some(session_id);
                view.screen = non_web_screen;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(!view.read(cx).can_open_tool_window());
            assert!(window.try_find("page-open-window").is_none());
        })
        .unwrap();
    }

    cx.update_window(window, |_, _window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.web_surface.invocation_id = Some(invocation_id.clone());
            view.web_surface.document = Some(builtin_doc.clone());
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).can_open_tool_window());
        assert!(window.try_find("page-open-window").is_some());
        window.click("page-open-window", cx);
    })
    .unwrap();
    let (child_handle, child) = cx.update(|cx| {
        let parent_view = view.read(cx);
        assert_eq!(parent_view.screen, Screen::Launcher);
        let (child_handle, child) = parent_view.tool_window_handles().next().unwrap();
        (child_handle, child.upgrade().unwrap())
    });
    cx.run_until_parked();
    cx.update_window(child_handle, |_, window, cx| {
        let child_view = child.read(cx);
        assert_eq!(child_view.screen, Screen::WebTool);
        assert!(child_view.standalone_tool);
        assert!(child_view
            .web_surface
            .document
            .as_ref()
            .is_some_and(|doc| doc.builtin && doc.handle != builtin_doc.handle));
        assert!(child_view
            .live
            .as_ref()
            .unwrap()
            .web_runtime()
            .application()
            .is_session_active(session_id));
        window.render_frame(cx);
        assert!(window.try_find("tool-window-titlebar").is_some());
        assert!(window.try_find("page-back").is_none());
        window.click("page-escape", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!cx.windows().contains(&child_handle));
        assert!(!child
            .read(cx)
            .live
            .as_ref()
            .unwrap()
            .web_runtime()
            .application()
            .is_session_active(session_id));
    });
}

#[gpui_kit::test]
fn tool_window_and_reopened_launcher_keep_independent_query_and_lifetime(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let package_path = directory.path().join("third-party.awplugin");
    write_test_awplugin(
        &package_path,
        "dev.example.third-party-tool",
        "Third Party Tool",
        "Open Third Party Tool",
    );
    let live = Arc::new(
        LiveLauncher::open_at(
            anywhere_core::application::frontend_runtime::RuntimePaths::new(
                directory.path().join("data"),
                plugins,
            ),
        )
        .unwrap(),
    );
    let LiveEvent::WebReview(review) = live
        .inspect_web_package(package_path.display().to_string())
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected WebReview");
    };
    assert!(!review.updating);
    let LiveEvent::WebPackages(_) = live
        .approve_web_package(review.token)
        .blocking_recv()
        .unwrap()
    else {
        panic!("expected WebPackages after approval");
    };

    let (id, invocation_id, document) = open_live_web_tool(
        &live,
        "Third Party Tool",
        "web:dev.example.third-party-tool:open",
    );
    assert!(!document.builtin);

    let (main, parent) = open_view(cx);
    cx.update_window(main, |_, _, cx| {
        parent.update(cx, |view, cx| {
            view.live = Some(live.clone());
            view.presence.presented = true;
            view.live_state.session_id = Some(id);
            view.screen = Screen::WebTool;
            view.web_surface.invocation_id = Some(invocation_id);
            view.web_surface.document = Some(document.clone());
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, main, &parent);
    cx.update_window(main, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("page-open-window").is_some());
        window.click("page-open-window", cx);
    })
    .unwrap();
    let (handle, child) = cx.update(|cx| {
        let view = parent.read(cx);
        assert_eq!(view.screen, Screen::Launcher);
        let (handle, child) = view.tool_window_handles().next().unwrap();
        (handle, child.upgrade().unwrap())
    });
    cx.run_until_parked();
    let new_session = live.reserve_activation().unwrap();
    cx.update_window(main, |_, window, cx| {
        parent.update(cx, |view, cx| {
            view.live_state.session_id = Some(new_session);
            set_input_value(&view.input, "separate", window, cx);
        });
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let view = child.read(cx);
        assert_eq!(view.screen, Screen::WebTool);
        assert!(view.standalone_tool);
        assert!(!view.dismiss_on_focus_loss());
        assert_ne!(
            view.web_surface
                .document
                .as_ref()
                .map(|doc| doc.handle.as_str()),
            Some(document.handle.as_str())
        );
        assert!(view
            .live
            .as_ref()
            .unwrap()
            .web_runtime()
            .application()
            .is_session_active(id));
        window.render_frame(cx);
        assert!(window.try_find("tool-window-titlebar").is_some());
        assert!(window.try_find("page-back").is_none());
        window.click("page-escape", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!cx.windows().contains(&handle));
        assert_eq!(parent.read(cx).input.read(cx).value(), "separate");
        assert!(live
            .web_runtime()
            .application()
            .is_session_active(new_session));
        assert!(!child
            .read(cx)
            .live
            .as_ref()
            .unwrap()
            .web_runtime()
            .application()
            .is_session_active(id));
    });
}

#[gpui_kit::test]
fn revoked_tool_window_exposes_reopen_state(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |view, cx| {
            view.standalone_tool = true;
            view.live_state.session_id = None;
            view.screen = Screen::UnsupportedTool;
            view.presence.presented = true;
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("expired-tool-window").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn returning_after_opening_a_web_tool_window_restores_the_launcher_entry(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.presence.presented = true;
            view.screen = Screen::WebTool;
            view.go_back(window, cx);
            assert_eq!(view.screen, Screen::Launcher);
        });
    })
    .unwrap();
}
