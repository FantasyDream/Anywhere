use super::*;
use crate::features::tools::ENTRY_CONTEXT;
use crate::{
    application::{Application, LauncherApplication},
    core::{
        action::descriptor::{ActionExecutor, ActionRegistry},
        context::{ContextSource, ContextValue, ResolvedContext},
        ranking::usage::MemoryUsage,
        search::{candidate::Candidate, engine::ActionSearch, query::Query},
        session::facts::SessionFacts,
    },
};
use anywhere_plugin::web::VerifiedWebPackage;
use anywhere_protocol::{web_plugin::*, *};
use std::{
    io::{Cursor, Write},
    sync::Arc,
    time::Duration,
};
use zip::{write::SimpleFileOptions, ZipWriter};

pub(super) fn package(
    permissions: serde_json::Value,
    accepts: serde_json::Value,
    body: &str,
) -> VerifiedWebPackage {
    package_named("dev.example.tool", permissions, accepts, body)
}
pub(super) fn package_named(
    id: &str,
    permissions: serde_json::Value,
    accepts: serde_json::Value,
    body: &str,
) -> VerifiedWebPackage {
    let manifest = serde_json::json!({"schemaVersion":1,"id":id,"name":"Example","version":"1.0.0","apiVersion":"1","platforms":["macos"],"entry":"index.html","permissions":permissions,"commands":[{"id":"open","title":"Text Pad","keywords":["scratchpad"],"accepts":accepts,"entrypoint":"workspace"}]});
    package_manifest(manifest, body)
}
pub(super) fn package_manifest(manifest: serde_json::Value, body: &str) -> VerifiedWebPackage {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("index.html", body.as_bytes().to_vec()),
    ] {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    VerifiedWebPackage::read(Cursor::new(zip.finish().unwrap().into_inner())).unwrap()
}

pub(super) fn approval(package: &VerifiedWebPackage) -> WebPackageApproval {
    WebPackageApproval {
        plugin_id: package.manifest().id.clone(),
        content_digest: package.digest().into(),
        permissions: package.manifest().permissions.clone(),
        source_id: "local:user-picked-package".into(),
    }
}
pub(super) fn catalog(package: &VerifiedWebPackage) -> WebPluginCatalog {
    WebPluginCatalog::prepare(WebPlatform::Macos, [(package, approval(package))]).unwrap()
}
pub(super) fn app() -> Application {
    Application::new(
        Arc::new(crate::application::source_catalog_tests::QuietPlatform),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    )
}
pub(super) fn source(binding: &WebDocumentBinding) -> WebMessageSource<'_> {
    WebMessageSource {
        runtime_id: binding.runtime_id(),
        document_epoch: binding.document_epoch(),
        origin: binding.origin(),
        main_frame: true,
    }
}
pub(super) fn context(value: &str) -> ResolvedContext {
    ResolvedContext::new(
        TEXT,
        ContextValue::Text(value.into()),
        ContextSource::SelectedText,
        "test",
    )
}
pub(super) fn offers(
    registry: &ActionRegistry,
    input: &str,
    contexts: &[ResolvedContext],
    scope: Option<&str>,
) -> Vec<Candidate> {
    let facts = SessionFacts::default();
    let search = ActionSearch::new(registry, &|_, _| Default::default(), &facts, 1000, |_| 0);
    let entries = crate::features::tools::EntrySource { search: &search };
    search
        .search_scoped(&Query::parse(input), contexts, &[&entries], scope)
        .candidates
}
pub(super) fn reference(
    registry: &ActionRegistry,
) -> crate::core::action::web_command::WebCommandRef {
    let action = registry.get("web:dev.example.tool:open").unwrap();
    let ActionExecutor::WebCommand(command) = &action.executor else {
        panic!("wrong executor");
    };
    command.clone()
}
pub(super) fn start(
    service: &mut WebPluginService,
    command: &crate::core::action::web_command::WebCommandRef,
    input: &ResolvedContext,
) -> String {
    let HostView::WebPlugin { invocation_id } =
        service.start(command, input, None, "test.app").unwrap()
    else {
        panic!("wrong view");
    };
    invocation_id
}
pub(super) fn service(package: &VerifiedWebPackage) -> (WebPluginService, ActionRegistry) {
    let catalog = catalog(package);
    let registry = catalog.append_to(&ActionRegistry::default()).unwrap();
    let mut service = WebPluginService::default();
    service.replace_catalog(catalog);
    (service, registry)
}

#[test]
fn catalog_requires_exact_review_and_platform_and_rejects_duplicate_ids() {
    let package = package(serde_json::json!([]), serde_json::json!([]), "hello");
    for which in 0..4 {
        let mut review = approval(&package);
        match which {
            0 => review.content_digest = "old".into(),
            1 => review.permissions.push(WebPermission::CurrentApp {}),
            2 => review.plugin_id = "dev.other".into(),
            _ => review.source_id.clear(),
        }
        assert!(WebPluginCatalog::prepare(WebPlatform::Macos, [(&package, review)]).is_err());
    }
    assert!(
        WebPluginCatalog::prepare(WebPlatform::Windows, [(&package, approval(&package))]).is_err()
    );
    assert!(WebPluginCatalog::prepare(
        WebPlatform::Macos,
        [
            (&package, approval(&package)),
            (&package, approval(&package))
        ]
    )
    .is_err());
}

#[test]
fn normal_search_never_exports_keywords_or_unmatched_text_fallbacks() {
    let package = package(
        serde_json::json!([{"name":"context.read","types":[TEXT]}]),
        serde_json::json!([TEXT]),
        "hello",
    );
    let (mut service, registry) = service(&package);
    let raw = ResolvedContext::new(
        TEXT,
        ContextValue::Text("scratchpad".into()),
        ContextSource::UserInput,
        "core.input",
    );
    assert!(offers(&registry, "unrelated", std::slice::from_ref(&raw), None).is_empty());
    let global = offers(&registry, "scratchpad", &[raw], None);
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].context().unwrap().type_id, ENTRY_CONTEXT);
    assert!(!global[0].presentation.home_eligible);
    assert_eq!(
        registry
            .persistent_binding("web:dev.example.tool:open")
            .unwrap()
            .action_id
            .as_str(),
        "web:dev.example.tool:open"
    );
    let id = start(
        &mut service,
        &reference(&registry),
        global[0].context().unwrap(),
    );
    let binding = service.bind(&id, WebPlatform::Macos).unwrap();
    assert_eq!(
        service
            .invocation(&binding, source(&binding))
            .unwrap()
            .input,
        WebInput::Empty {
            reason: WebInputReason::NoObject
        }
    );
    let selected = offers(&registry, "Text Pad", &[context("actual selection")], None);
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].context().unwrap().value.text(),
        "actual selection"
    );
    assert!(offers(&registry, "Text Pad", &[], Some("git")).is_empty());
}

#[test]
fn context_projection_is_permission_scoped_and_does_not_leak_paths_or_properties() {
    let input = context("secret");
    assert_eq!(
        projection::build_with_resources(&input, &[]).unwrap().0,
        WebInput::Empty {
            reason: WebInputReason::PermissionDenied
        }
    );
    let grant = [WebPermission::ContextRead {
        types: vec![TEXT.into(), DIRECTORY.into(), FILE_SELECTION.into()],
    }];
    assert_eq!(
        projection::build_with_resources(&input, &grant).unwrap().0,
        WebInput::Text {
            source: ContextSource::SelectedText,
            text: "secret".into()
        }
    );
    let large = context(&"x".repeat(anywhere_protocol::web_bridge::MAX_TEXT_BYTES + 1));
    assert_eq!(
        projection::build_with_resources(&large, &grant).unwrap().0,
        WebInput::Empty {
            reason: WebInputReason::TooLarge
        }
    );
    let directory = ResolvedContext::new(
        DIRECTORY,
        ContextValue::Directory("/private/project/repo".into()),
        ContextSource::FinderSelection,
        "test",
    );
    let (input, handles) = projection::build_with_resources(&directory, &grant).unwrap();
    let json = serde_json::to_string(&input).unwrap();
    assert!(!json.contains("/private"));
    let WebInput::Resources { items, .. } = input else {
        panic!("no resource");
    };
    assert_eq!(items[0].name, "repo");
    assert_eq!(
        handles[&items[0].handle].to_str(),
        Some("/private/project/repo")
    );
    let selection = ResolvedContext::new(
        FILE_SELECTION,
        ContextValue::FileSelection(vec!["/a".into(); 129].into()),
        ContextSource::FinderSelection,
        "test",
    );
    assert_eq!(
        projection::build_with_resources(&selection, &grant)
            .unwrap()
            .0,
        WebInput::Empty {
            reason: WebInputReason::TooLarge
        }
    );
    let mut invalid = directory;
    invalid.value = ContextValue::Extension("must not coerce to a path".into());
    assert_eq!(
        projection::build_with_resources(&invalid, &grant)
            .unwrap()
            .0,
        WebInput::Empty {
            reason: WebInputReason::UnsupportedType
        }
    );
}

#[test]
fn source_frame_runtime_epoch_and_reload_are_qualified() {
    let package = package(serde_json::json!([]), serde_json::json!([TEXT]), "hello");
    let (mut service, registry) = service(&package);
    let id = start(&mut service, &reference(&registry), &context("secret"));
    let binding = service.bind(&id, WebPlatform::Macos).unwrap();
    for which in 0..4 {
        let mut wrong = source(&binding);
        match which {
            0 => wrong.origin = "https://evil.test",
            1 => wrong.main_frame = false,
            2 => wrong.document_epoch += 1,
            _ => wrong.runtime_id = "other-view",
        }
        assert!(service.invocation(&binding, wrong).is_err());
    }
    let replacement = service.bind(&id, WebPlatform::Macos).unwrap();
    assert_ne!(replacement.runtime_id(), binding.runtime_id());
    assert!(service.invocation(&binding, source(&binding)).is_err());
    assert!(service.invocation(&replacement, source(&binding)).is_err());
    assert!(service
        .invocation(&replacement, source(&replacement))
        .is_ok());
    let another = start(&mut service, &reference(&registry), &context("other"));
    assert_ne!(id, another);
    assert!(service
        .invocation(&replacement, source(&replacement))
        .is_err());
    let binding = service.bind(&another, WebPlatform::Macos).unwrap();
    assert!(service.current_app(&binding, source(&binding)).is_err());
    service.close_document(&binding);
    assert!(service.invocation(&binding, source(&binding)).is_err());
}

#[tokio::test(start_paused = true)]
async fn hide_revokes_immediately_preserves_input_below_15s_and_expires_at_15s() {
    let package = package(
        serde_json::json!([{"name":"context.read","types":[TEXT]}]),
        serde_json::json!([TEXT]),
        "hello",
    );
    let (mut service, registry) = service(&package);
    let id = start(&mut service, &reference(&registry), &context("keep me"));
    let binding = service.bind(&id, WebPlatform::Macos).unwrap();
    service.suspend();
    assert!(service.invocation(&binding, source(&binding)).is_err());
    tokio::time::advance(Duration::from_secs(14)).await;
    let resumed = service.bind(&id, WebPlatform::Macos).unwrap();
    assert_eq!(
        service
            .invocation(&resumed, source(&resumed))
            .unwrap()
            .input,
        WebInput::Text {
            source: ContextSource::SelectedText,
            text: "keep me".into()
        }
    );
    service.suspend();
    tokio::time::advance(Duration::from_secs(14)).await;
    service.suspend(); // Does not extend retention.
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(service.bind(&id, WebPlatform::Macos).is_err());
}

#[test]
fn update_revokes_references_documents_and_same_ids_have_new_qualifications() {
    let old = package(serde_json::json!([]), serde_json::json!([TEXT]), "old");
    let new = package(serde_json::json!([]), serde_json::json!([TEXT]), "new JS");
    let (mut service, registry) = service(&old);
    let command = reference(&registry);
    let id = start(&mut service, &command, &context("old"));
    let binding = service.bind(&id, WebPlatform::Macos).unwrap();
    let next = catalog(&new);
    let updated = service.prepare_registry(&next, &registry).unwrap();
    assert_eq!(command.action_id(), reference(&updated).action_id());
    assert_ne!(command, reference(&updated));
    service.replace_catalog(next);
    assert!(service
        .start(&command, &context("old"), None, "test")
        .is_err());
    assert!(service.invocation(&binding, source(&binding)).is_err());
    assert!(service
        .start(&reference(&updated), &context("new"), None, "test")
        .is_ok());
}

#[test]
fn source_replacement_and_registration_collision_fail_without_partial_publication() {
    let package = package(serde_json::json!([]), serde_json::json!([TEXT]), "hello");
    let (service, registry) = service(&package);
    let mut changed = approval(&package);
    changed.source_id = "different-publisher".into();
    let next = WebPluginCatalog::prepare(WebPlatform::Macos, [(&package, changed)]).unwrap();
    assert!(service.prepare_registry(&next, &registry).is_err());
    let mut conflict = ActionRegistry::default();
    let definition = crate::core::action::descriptor::ActionDefinition {
        id: "web:dev.example.tool:open".into(),
        title: "collision".into(),
        subtitle: None,
        shortcut: None,
        accepts: vec![],
        executor: ActionExecutor::Declarative(DeclarativeAction::Copy { text: "x".into() }),
        availability: Default::default(),
        base_priority: 0,
        icon: "code".into(),
        group_id: "tools".into(),
        aliases: vec![],
        keywords: vec![],
        owning_app: None,
        menu_path: vec![],
        home_eligible: false,
        primary_action: false,
        search_title: None,
    };
    conflict.register(definition).unwrap();
    assert!(service
        .prepare_registry(&catalog(&package), &conflict)
        .is_err());
    assert_eq!(conflict.commands().count(), 1);
    assert_eq!(
        reference(&registry).action_id(),
        "web:dev.example.tool:open"
    );
}

#[tokio::test]
async fn application_executes_committed_candidate_records_only_ready_and_revokes_on_activation() {
    let app = app();
    let package = package(
        serde_json::json!([{"name":"context.currentApp"}]),
        serde_json::json!([]),
        "hello",
    );
    app.publish_web_catalog(catalog(&package)).unwrap();
    let session_id = app.begin_activation().unwrap();
    let registry = app.resolver.base_registry();
    let candidate = offers(&registry, "Text Pad", &[], None).remove(0);
    let candidate_id = candidate.presentation.id.clone();
    {
        let mut session = app.session().unwrap();
        session.identity.source_app = "before.app".into();
        session.identity.frontmost = Some(anywhere_platform::ApplicationInfo {
            bundle_id: "before.app".into(),
            name: "Before".into(),
            pid: 42,
            instance_id: Some("private process identity".into()),
        });
        session.execution.revision = Some(0);
        session.execution.action_catalog_version = Some(
            app.resolver
                .action_catalog_version
                .load(std::sync::atomic::Ordering::Acquire),
        );
        session
            .execution
            .candidates
            .insert(candidate_id.clone().into(), candidate);
    }
    let result = app.execute(session_id, 0, candidate_id).await.unwrap();
    assert!(result.succeeded(), "{:?}", result.error);
    assert!(!result.close_window);
    let HostView::WebPlugin { invocation_id } = result.view.unwrap() else {
        panic!("wrong view");
    };
    assert!(app
        .resolver
        .history
        .lock()
        .unwrap()
        .load_global()
        .unwrap()
        .is_empty());
    let binding = app
        .bind_web_document(&invocation_id, WebPlatform::Macos)
        .unwrap();
    assert_eq!(
        serde_json::from_value::<Option<WebCurrentApp>>(
            rpc_value(&app, &binding, "app", "context.getCurrentApp").await
        )
        .unwrap(),
        Some(WebCurrentApp {
            id: "before.app".into(),
            name: "Before".into()
        })
    );
    assert_eq!(binding.content_digest(), package.digest());
    for id in ["ready1", "ready2"] {
        rpc_value(&app, &binding, id, "lifecycle.ready").await;
    }
    let reloaded = app
        .bind_web_document(&invocation_id, WebPlatform::Macos)
        .unwrap();
    rpc_value(&app, &reloaded, "ready3", "lifecycle.ready").await;
    let usage = app.resolver.history.lock().unwrap().load_global().unwrap();
    assert_eq!(
        usage[&("web:dev.example.tool:open".into(), ENTRY_CONTEXT.into())].execute_count,
        1
    );
    app.suspend_session(session_id).await.unwrap();
    assert!(app
        .bind_web_document(&invocation_id, WebPlatform::Macos)
        .is_err());
    assert!(app.web_invocation(&reloaded, source(&reloaded)).is_err());
    app.begin_activation().unwrap();
    assert!(app
        .bind_web_document(&invocation_id, WebPlatform::Macos)
        .is_err());
}

#[test]
fn current_app_aggregates_context_commands_and_global_tools_do_not_create_scopes() {
    let package = package(
        serde_json::json!([]),
        serde_json::json!([DIRECTORY]),
        "hello",
    );
    let (_, registry) = service(&package);
    let context = ResolvedContext::new(
        DIRECTORY,
        ContextValue::Directory("/selected".into()),
        ContextSource::FinderSelection,
        "test",
    );
    let mut facts = SessionFacts::default();
    facts.frontmost = Some(anywhere_platform::ApplicationInfo {
        bundle_id: "captured.app".into(),
        name: "Captured".into(),
        pid: 0,
        instance_id: None,
    });
    let search = ActionSearch::new(&registry, &|_, _| Default::default(), &facts, 0, |_| 0);
    let results = search.search_scoped(
        &Query::parse("Text Pad"),
        &[context],
        &[],
        Some("app.captured.app"),
    );
    assert_eq!(results.candidates.len(), 1);
    assert_eq!(results.candidates[0].context().unwrap().type_id, DIRECTORY);
    assert!(offers(&registry, "", &[], None).is_empty());
    let shortcuts = crate::features::tools::EntrySource { search: &search }.binding_offers();
    assert_eq!(shortcuts.len(), 1);
    assert_eq!(shortcuts[0].context().unwrap().type_id, ENTRY_CONTEXT);
}

#[tokio::test]
async fn source_policy_changes_and_catalog_removal_revoke_issued_documents() {
    let app = app();
    let package = package(serde_json::json!([]), serde_json::json!([TEXT]), "hello");
    app.publish_web_catalog(catalog(&package)).unwrap();
    app.begin_activation().unwrap();
    let command = reference(&app.resolver.base_registry());
    let id = start(
        &mut app.web_plugins.lock().unwrap(),
        &command,
        &context("selected"),
    );
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    let mut changed = approval(&package);
    changed.source_id = "unreviewed-new-source".into();
    let replacement = WebPluginCatalog::prepare(WebPlatform::Macos, [(&package, changed)]).unwrap();
    assert!(app.publish_web_catalog(replacement).is_err());
    assert!(app.web_invocation(&binding, source(&binding)).is_ok());
    app.set_capture_sources(false, false, false, false);
    assert!(app.web_invocation(&binding, source(&binding)).is_err());
    let id = start(
        &mut app.web_plugins.lock().unwrap(),
        &command,
        &context("another"),
    );
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    app.publish_web_catalog(WebPluginCatalog::default())
        .unwrap();
    assert!(app.web_invocation(&binding, source(&binding)).is_err());
    assert!(app
        .resolver
        .base_registry()
        .get(&command.action_id())
        .is_none());
}

#[tokio::test(start_paused = true)]
async fn delayed_disposal_cannot_close_a_reloaded_or_resumed_document() {
    let package = package(serde_json::json!([]), serde_json::json!([TEXT]), "hello");
    let (mut service, registry) = service(&package);
    let id = start(&mut service, &reference(&registry), &context("draft"));
    let first = service.bind(&id, WebPlatform::Macos).unwrap();
    let reloaded = service.bind(&id, WebPlatform::Macos).unwrap();
    service.close_document(&first);
    assert!(service.invocation(&reloaded, source(&reloaded)).is_ok());
    service.suspend();
    let resumed = service.resume(&reloaded, WebPlatform::Macos).unwrap();
    service.close_document(&reloaded);
    assert!(service.invocation(&resumed, source(&resumed)).is_ok());
    service.suspend();
    service.close_document(&resumed);
    assert!(service.resume(&resumed, WebPlatform::Macos).is_err());
}

/// Exercise the same capability and usage path as the SDK, not a parallel facade.
pub(super) async fn rpc_value(
    app: &Application,
    binding: &WebDocumentBinding,
    id: &str,
    method: &str,
) -> serde_json::Value {
    let frame =
        serde_json::json!({"protocol":1,"requestId":id,"method":method,"params":{}}).to_string();
    let WebDispatch::Reply(reply) = app
        .dispatch_web_message(binding, source(binding), &frame)
        .await
    else {
        panic!("unexpected confirmation");
    };
    let result: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert!(result.get("error").is_none(), "{result}");
    result["result"]["value"].clone()
}
