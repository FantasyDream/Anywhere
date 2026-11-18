//! Built-in module identity and capability-gated host services.
use crate::support::*;
use anywhere_core::core::action::capabilities::{CapabilityGrant, HostServices};
use anywhere_protocol::extension::ExtensionCapability as Capability;

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
            environment: Default::default(),
        })
        .is_err());
    assert_eq!(platform.git_calls.load(Ordering::Relaxed), 0);
    assert!(platform.calls.lock().unwrap().is_empty());
    host.copy_text("approved").unwrap();
    assert_eq!(
        *platform.calls.lock().unwrap(),
        [("copy".into(), "approved".into())]
    );
}
