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
async fn two_first_party_modules_keep_context_action_scope_and_execution_identity() {
    let modules = anywhere_core::extensions::builtins().unwrap();
    assert_eq!(
        modules.iter().map(|m| m.id).collect::<Vec<_>>(),
        ["url", "json"]
    );
    let platform = Arc::new(TestPlatform::default());
    let app = Application::new(
        platform.clone(),
        registry(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    let id = app.activate().await.unwrap().unwrap().session_id;
    app.update_input(id, r#"{"value":1}"#.into(), 1)
        .await
        .unwrap();
    let state = settled(&app, id, 1).await;
    assert!(state
        .context_summary
        .iter()
        .any(|c| c.type_id == JSON_DOCUMENT));
    let formatted = app
        .execute(id, 1, candidate(&state, "core.json.format"))
        .await
        .unwrap();
    assert_eq!(formatted.status, ActionStatus::Succeeded);
    assert!(formatted.output_text.as_deref().unwrap().contains("\n"));
    app.execute(id, 1, candidate(&state, "core.json.copy"))
        .await
        .unwrap();
    app.update_input(id, "https://example.com/project".into(), 2)
        .await
        .unwrap();
    let state = settled(&app, id, 2).await;
    assert!(state.scopes.iter().any(|s| s.id == "web"));
    app.execute(id, 2, candidate(&state, "core.url.open"))
        .await
        .unwrap();
    let calls = platform.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "copy");
    assert_eq!(
        calls[1],
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
