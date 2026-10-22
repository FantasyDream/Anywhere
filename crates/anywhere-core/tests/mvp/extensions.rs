//! Extension migration and denied/granted/revoked execution through the Facade.
use crate::support::*;
use anywhere_core::core::action::{
    capabilities::{CapabilityGrant, HostServices},
    descriptor::ActionExecutor,
};
use anywhere_protocol::extension::ExtensionCapability as Capability;
const COPY_V2: &str = r#"protocol_version = 2
version = "0.8.0"
capabilities = ["clipboard.write"]
id = "scoped"
name = "Scoped Copy"
[[commands]]
id = "copy"
title = "Scoped Copy"
accepts = ["core.text"]
action = {type = "copy", text = "{{context.value}}"}
"#;

#[tokio::test]
async fn delivered_modules_keep_catalog_and_url_execution_identity() {
    let modules = anywhere_core::features::builtin_contributions().unwrap();
    assert_eq!(
        modules
            .iter()
            .filter(|m| matches!(m.id, "git" | "url"))
            .map(|m| m.id)
            .collect::<Vec<_>>(),
        ["git", "url"]
    );
    let platform = Arc::new(TestPlatform::default());
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "https://example.com/project".into(), 2)
        .await
        .unwrap();
    let state = settled(&app, id, 2).await;
    assert!(state.scopes.iter().any(|s| s.id == "web"));
    app.execute(id, 2, candidate(&state, "core.url.open"))
        .await
        .unwrap();
    let calls = platform.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        ("url".into(), "https://example.com/project".into())
    );
}

#[tokio::test]
async fn v2_requires_host_grants_and_reload_revokes_old_candidates() {
    let manifest = anywhere_plugin::parse_manifest(COPY_V2).unwrap();
    let mut denied = registry();
    assert!(denied
        .register_plugin(manifest.clone())
        .unwrap_err()
        .to_string()
        .contains("not granted"));
    assert!(denied.get("scoped.copy").is_none());
    let mut granted = registry();
    granted
        .register_plugin_with_capabilities(manifest, &[Capability::ClipboardWrite])
        .unwrap();
    let platform = Arc::new(TestPlatform::default());
    let app = Application::new(
        platform.clone(),
        granted,
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "hello extension".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    let row = candidate(&state, "scoped.copy");
    assert!(state.candidates.iter().any(|c| c.id == row && !c.is_menu));
    app.execute(id, 1, row.clone()).await.unwrap();
    assert_eq!(
        *platform.calls.lock().unwrap(),
        [("copy".into(), "hello extension".into())]
    );
    app.replace_registry(registry());
    assert!(app.execute(id, 1, row).await.is_err());
    assert_eq!(platform.calls.lock().unwrap().len(), 1);
}

#[test]
fn runtime_host_services_deny_operations_before_any_side_effect() {
    let platform = TestPlatform::default();
    let grant =
        CapabilityGrant::approve(&[Capability::ClipboardWrite], &[Capability::ClipboardWrite])
            .unwrap();
    let host = HostServices::new(&platform, &grant);
    assert!(host.open_url("https://example.com").is_err());
    assert!(host.open_file(Path::new("/not-opened")).is_err());
    assert!(host
        .run_git(Path::new("/not-read"), GitOperation::Status)
        .is_err());
    assert!(host
        .process_request(anywhere_platform::process::ProcessRequest {
            title: "Denied".into(),
            program: "/not-run".into(),
            arguments: vec![],
            directory: "/not-run".into(),
            environment: Default::default()
        })
        .is_err());
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
    assert!(platform.calls.lock().unwrap().is_empty());
    let context = ResolvedContext::new(
        TEXT,
        ContextValue::Text("hello".into()),
        ContextSource::UserInput,
        "test",
    );
    let forged = ActionExecutor::ScopedDeclarative {
        action: DeclarativeAction::Copy {
            text: "{{context.value}}".into(),
        },
        grant: CapabilityGrant::default(),
    };
    assert!(anywhere_core::core::action::execute::execute(
        &forged,
        &context,
        &platform,
        &registry()
    )
    .is_err());
    assert!(platform.calls.lock().unwrap().is_empty());
    host.copy_text("approved").unwrap();
    assert_eq!(
        *platform.calls.lock().unwrap(),
        [("copy".into(), "approved".into())]
    );
}

#[test]
fn legacy_raw_dto_and_parser_migration_remain_registration_compatible() {
    let raw: PluginManifest = toml::from_str(anywhere_plugin::EXAMPLE_PLUGIN).unwrap();
    let migrated = anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap();
    let mut before = anywhere_core::application::builtin_registry();
    let mut after = anywhere_core::application::builtin_registry();
    before.register_plugin(raw).unwrap();
    after.register_plugin(migrated).unwrap();
    let left = before.get("example.search").unwrap();
    let right = after.get("example.search").unwrap();
    assert_eq!(left.id, right.id);
    assert_eq!(left.title, right.title);
    assert_eq!(left.group_id, right.group_id);
    assert_eq!(left.accepts, right.accepts);
}

#[tokio::test]
async fn approved_ui_returns_structured_data_and_denies_stale_execution() {
    let manifest = anywhere_plugin::parse_manifest(include_str!(
        "../../../../plugins/examples/context-detail.toml"
    ))
    .unwrap();
    let mut denied = registry();
    assert!(denied.register_plugin(manifest.clone()).is_err());
    let mut granted = registry();
    granted
        .register_plugin_with_capabilities(manifest, &[Capability::RenderUi])
        .unwrap();
    let platform = Arc::new(TestPlatform::default());
    let app = Application::new(
        platform.clone(),
        granted,
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, "hello extension".into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    let row = candidate(&state, "example_detail.inspect");
    let result = app.execute(id, 1, row.clone()).await.unwrap();
    assert_eq!(result.status, ActionStatus::Succeeded);
    assert!(!result.close_window);
    assert!(result.output_text.is_none());
    let HostView::PluginDetail { detail } = result.view.unwrap() else {
        panic!("expected detail")
    };
    assert_eq!(detail.rows[0].value, "hello extension");
    assert!(platform.calls.lock().unwrap().is_empty());
    app.update_input(id, "x".repeat(16 * 1024 + 1), 2)
        .await
        .unwrap();
    let state = settled(&app, id, 2).await;
    let oversized = app
        .execute(id, 2, candidate(&state, "example_detail.inspect"))
        .await
        .unwrap();
    assert_eq!(oversized.status, ActionStatus::Failed);
    assert!(oversized.view.is_none());
    assert!(!oversized.close_window);
    assert!(app.execute(id, 1, row).await.is_err());
    let row = candidate(&state, "example_detail.inspect");
    app.suspend_session(id).await.unwrap();
    app.replace_registry(registry());
    app.resume_session(id).unwrap();
    assert!(app.execute(id, 2, row).await.is_err());
    assert!(platform.calls.lock().unwrap().is_empty());
}

#[test]
fn ui_executor_checks_grants_even_for_directly_constructed_dtos() {
    let manifest = anywhere_plugin::parse_manifest(include_str!(
        "../../../../plugins/examples/context-detail.toml"
    ))
    .unwrap();
    let context = ResolvedContext::new(
        TEXT,
        ContextValue::Text("hello".into()),
        ContextSource::UserInput,
        "test",
    );
    let platform = TestPlatform::default();
    for executor in [
        ActionExecutor::ScopedDeclarative {
            action: manifest.commands[0].action.clone(),
            grant: CapabilityGrant::default(),
        },
        ActionExecutor::Declarative(manifest.commands[0].action.clone()),
    ] {
        assert!(anywhere_core::core::action::execute::execute(
            &executor,
            &context,
            &platform,
            &registry()
        )
        .is_err());
    }
    assert!(platform.calls.lock().unwrap().is_empty());
}
