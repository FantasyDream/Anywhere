use super::{tests::*, *};
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use anywhere_protocol::{web_plugin::*, *};
use serde_json::json;

fn json_package() -> anywhere_plugin::web::VerifiedWebPackage {
    // Use the shipped example declaration so removal of its matcher/permissions
    // breaks the same tests that exercise the framework, not just a fixture.
    let mut manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../plugins/examples/json-workspace/manifest.json"
    ))
    .unwrap();
    manifest["entry"] = json!("index.html");
    package_manifest(manifest, "<p>synthetic</p>")
}

fn raw(text: &str, source: ContextSource) -> ResolvedContext {
    ResolvedContext::new(TEXT, ContextValue::Text(text.into()), source, "test.input")
}

#[test]
fn typing_json_offers_copy_actions_and_invocation_keeps_exact_text() {
    let package = json_package();
    let (mut service, registry) = service(&package);
    let text = " \n{\"n\":123456789012345678901,\"n\":1e9999,\"text\":\"中文\\n\"}\t";
    let input = raw(text, ContextSource::UserInput);
    let found = offers(&registry, text, std::slice::from_ref(&input), None);
    assert_eq!(found.len(), 4);
    assert!(!found
        .iter()
        .any(|offer| offer.presentation.title == "Unescape JSON"));
    let workspace = found
        .iter()
        .find(|offer| offer.presentation.title == "JSON Workspace")
        .unwrap();
    assert_eq!(workspace.context().unwrap().value.text(), text);
    let crate::core::action::descriptor::ActionExecutor::WebCommand(command) = &registry
        .get("web:dev.anywhere.json-workspace:open")
        .unwrap()
        .executor
    else {
        panic!()
    };
    let id = start(&mut service, command, workspace.context().unwrap());
    let binding = service.bind(&id, WebPlatform::Macos).unwrap();
    assert_eq!(
        service
            .invocation(&binding, source(&binding))
            .unwrap()
            .input,
        WebInput::Text {
            source: ContextSource::UserInput,
            text: text.into()
        }
    );
    assert!(service
        .start(
            command,
            &raw("search term", ContextSource::UserInput),
            None,
            ""
        )
        .is_err());
    assert!(offers(&registry, text, &[input], Some("git")).is_empty());
}

#[test]
fn unrecognized_text_is_not_suggested_but_keywords_still_open_an_empty_workspace() {
    let package = json_package();
    let (_, registry) = service(&package);
    for text in [
        "123",
        "true",
        "null",
        "\"text\"",
        "ordinary text",
        "{broken}",
        "[1,]",
    ] {
        assert!(offers(
            &registry,
            text,
            &[raw(text, ContextSource::UserInput)],
            None
        )
        .is_empty());
        assert!(offers(
            &registry,
            "",
            &[raw(text, ContextSource::SelectedText)],
            None
        )
        .is_empty());
    }
    for keyword in ["json", "JSON", "format", "格式化"] {
        let results = offers(
            &registry,
            keyword,
            &[raw(keyword, ContextSource::UserInput)],
            None,
        );
        assert_eq!(results.len(), 1, "keyword: {keyword}");
        assert_eq!(results[0].presentation.title, "JSON Workspace");
        assert!(
            results
                .iter()
                .all(|offer| offer.context().unwrap().type_id
                    == crate::features::tools::ENTRY_CONTEXT)
        );
    }
    let selected = raw("[]", ContextSource::SelectedText);
    assert_eq!(
        offers(&registry, "", std::slice::from_ref(&selected), None).len(),
        4
    );
    assert!(offers(&registry, "unrelated", &[selected], None).is_empty());
}

#[test]
fn json_helpers_require_matching_content_even_with_keywords_or_shortcuts() {
    let (_, registry) = service(&json_package());
    for source in [ContextSource::SelectedText, ContextSource::Clipboard] {
        for text in ["ordinary text", "{broken}", "123"] {
            let input = raw(text, source);
            let results = offers(&registry, "json", std::slice::from_ref(&input), None);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].presentation.title, "JSON Workspace");
            for keyword in ["copy", "minify", "escape", "unescape", "复制"] {
                assert!(offers(&registry, keyword, std::slice::from_ref(&input), None).is_empty());
            }
        }
        assert_eq!(offers(&registry, "", &[raw("[]", source)], None).len(), 4);
        let escaped = serde_json::to_string("[]").unwrap();
        assert_eq!(
            offers(&registry, "", &[raw(&escaped, source)], None).len(),
            5
        );
    }
    let facts = crate::core::session::facts::SessionFacts::default();
    let search = crate::core::search::engine::ActionSearch::new(
        &registry,
        &|_, _| Default::default(),
        &facts,
        1000,
        |_| 0,
    );
    let bindings = crate::features::tools::EntrySource { search: &search }.binding_offers(&[]);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].presentation.title, "JSON Workspace");
}

#[test]
fn json_helpers_reject_unmatched_execution_without_starting_a_webview() {
    let (mut service, registry) = service(&json_package());
    let workspace = ResolvedContext::new(
        crate::features::tools::ENTRY_CONTEXT,
        ContextValue::Text("".into()),
        ContextSource::ForegroundApplication,
        "core.launcher",
    );
    for id in ["copy-format", "copy-minify", "copy-escaped", "unescape"] {
        let action = registry
            .get(&format!("web:dev.anywhere.json-workspace:{id}"))
            .unwrap();
        let crate::core::action::descriptor::ActionExecutor::WebCommand(command) = &action.executor
        else {
            panic!("expected Web command");
        };
        for input in [
            workspace.clone(),
            raw("json", ContextSource::UserInput),
            raw("{broken}", ContextSource::SelectedText),
            raw("ordinary text", ContextSource::Clipboard),
        ] {
            assert!(service.start(command, &input, None, "").is_err(), "{id}");
        }
        if id == "unescape" {
            assert!(service
                .start(command, &raw("{}", ContextSource::SelectedText), None, "")
                .is_err());
        }
    }
    assert!(service.invocations.is_empty());
}

fn capture(source: ContextSource, text: &str, age: u64) -> ContextCapturePresentation {
    ContextCapturePresentation {
        source,
        status: ContextCaptureStatus::Captured,
        captured_at_ms: 1000,
        clipboard_age_ms: Some(age),
        text: Some(text.into()),
    }
}

#[tokio::test]
async fn activation_uses_enabled_catalog_and_keeps_selection_priority_and_clipboard_expiry() {
    let app = app();
    let package = json_package();
    let copied = capture(ContextSource::Clipboard, "{\"copied\":1}", 0);
    assert!(app
        .choose_activation_input(std::slice::from_ref(&copied), 1000, false, true, true)
        .is_none());
    app.publish_web_catalog(catalog(&package)).unwrap();
    assert!(app
        .choose_activation_input(std::slice::from_ref(&copied), 16000, false, true, true)
        .is_some());
    assert!(app
        .choose_activation_input(std::slice::from_ref(&copied), 16001, false, true, true)
        .is_none());
    let selected = capture(ContextSource::SelectedText, "[1,2]", 0);
    assert_eq!(
        app.choose_activation_input(&[selected, copied.clone()], 1000, true, true, false)
            .unwrap()
            .source,
        ContextSource::SelectedText
    );
    for text in ["ordinary text", "{broken", "123"] {
        let selected = capture(ContextSource::SelectedText, text, 0);
        assert_eq!(
            app.choose_activation_input(&[selected, copied.clone()], 1000, true, true, false)
                .unwrap()
                .source,
            ContextSource::Clipboard
        );
    }
    assert!(app.web_plugins.lock().unwrap().invocations.is_empty());
    app.publish_web_catalog(WebPluginCatalog::default())
        .unwrap();
    assert!(app
        .choose_activation_input(&[copied], 1000, false, true, true)
        .is_none());
}

#[test]
fn persisted_disable_and_reenable_control_recognition_after_reopen() {
    use crate::application::web_plugins::WebPackageStore;
    let temp = tempfile::tempdir().unwrap();
    let app = app();
    let package = json_package();
    let path = temp.path().join("json.awplugin");
    package
        .write_archive(std::fs::File::create(&path).unwrap())
        .unwrap();
    let root = temp.path().join("store");
    let mut store = WebPackageStore::open(root.clone(), WebPlatform::Macos, &app).unwrap();
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    assert!(app.resolver.base_registry().recognizes_web_input("{}"));
    store.set_enabled(&app, &review.id, false).unwrap();
    assert!(!app.resolver.base_registry().recognizes_web_input("{}"));
    drop(store);
    let mut store = WebPackageStore::open(root, WebPlatform::Macos, &app).unwrap();
    assert!(!app.resolver.base_registry().recognizes_web_input("{}"));
    store.set_enabled(&app, &review.id, true).unwrap();
    assert!(app.resolver.base_registry().recognizes_web_input("{}"));
    store.uninstall(&app, &review.id).unwrap();
    assert!(!app.resolver.base_registry().recognizes_web_input("{}"));
}

#[tokio::test]
async fn launcher_search_and_content_mode_execute_the_committed_json_candidate() {
    use crate::application::LauncherApplication;
    let app = app();
    app.publish_web_catalog(catalog(&json_package())).unwrap();
    let session = app.begin_activation().unwrap();
    let keyword = app
        .update_context_input(session, "JSON".into(), 1, None, InputMode::Search)
        .await
        .unwrap()
        .unwrap();
    let commands: Vec<_> = keyword
        .candidates
        .iter()
        .filter_map(|candidate| candidate.command_id.as_deref())
        .filter(|id| id.starts_with("web:dev.anywhere.json-workspace:"))
        .collect();
    assert_eq!(commands, ["web:dev.anywhere.json-workspace:open"]);
    for (revision, mode) in [(2, InputMode::Search), (3, InputMode::Content)] {
        let input = " {\"numbers\":[1,2,3]} ";
        let state = app
            .update_context_input(session, input.into(), revision, None, mode)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            state.candidates[0].command_id.as_deref(),
            Some("web:dev.anywhere.json-workspace:open"),
            "fresh JSON input should default to Workspace, not a copy helper"
        );
        let candidate = state
            .candidates
            .iter()
            .find(|candidate| {
                candidate.command_id.as_deref() == Some("web:dev.anywhere.json-workspace:open")
            })
            .expect("JSON Action must reach the real Launcher snapshot");
        let result = app
            .execute(session, revision, candidate.id.clone())
            .await
            .unwrap();
        assert!(result.succeeded(), "{:?}", result.error);
        let HostView::WebPlugin { invocation_id } = result.view.unwrap() else {
            panic!()
        };
        let binding = app
            .bind_web_document(&invocation_id, WebPlatform::Macos)
            .unwrap();
        assert_eq!(
            app.web_invocation(&binding, source(&binding))
                .unwrap()
                .input,
            WebInput::Text {
                source: ContextSource::UserInput,
                text: input.into()
            }
        );
    }
}

#[tokio::test]
async fn escaped_json_actions_keep_the_exact_invocation_and_route_each_entrypoint() {
    use crate::application::LauncherApplication;
    let app = app();
    app.publish_web_catalog(catalog(&json_package())).unwrap();
    let encoded = serde_json::to_string(r#"{"n":900719925474099312345,"s":"line\n"}"#).unwrap();
    let selected = capture(ContextSource::SelectedText, &encoded, 0);
    assert!(app
        .choose_activation_input(&[selected], 1000, true, true, false)
        .is_some());
    assert!(app.web_plugins.lock().unwrap().invocations.is_empty());
    let session = app.begin_activation().unwrap();
    for (index, command) in [
        "open",
        "copy-format",
        "copy-minify",
        "copy-escaped",
        "unescape",
    ]
    .into_iter()
    .enumerate()
    {
        let revision = index as u32 + 1;
        let state = app
            .update_context_input(session, encoded.clone(), revision, None, InputMode::Content)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.candidates.len(), 5);
        let command_id = format!("web:dev.anywhere.json-workspace:{command}");
        let candidate = state
            .candidates
            .iter()
            .find(|item| item.command_id.as_deref() == Some(command_id.as_str()))
            .unwrap();
        let result = app
            .execute(session, revision, candidate.id.clone())
            .await
            .unwrap();
        assert!(result.succeeded());
        let HostView::WebPlugin { invocation_id } = result.view.unwrap() else {
            panic!()
        };
        let binding = app
            .bind_web_document(&invocation_id, WebPlatform::Macos)
            .unwrap();
        let invocation = app.web_invocation(&binding, source(&binding)).unwrap();
        assert_eq!(
            invocation.entrypoint,
            if command == "open" {
                "workspace"
            } else {
                command
            }
        );
        assert_eq!(
            invocation.input,
            WebInput::Text {
                source: ContextSource::UserInput,
                text: encoded.clone()
            }
        );
    }
}
