use super::{tests::*, *};
use crate::core::context::{ContextValue, ExecutionContext, ResolvedContext};
use crate::Application;
use anywhere_protocol::{web_plugin::*, *};
use serde_json::{json, Value};
use std::{
    io::{Cursor, Write},
    sync::Arc,
    time::Duration,
};
use zip::{write::SimpleFileOptions, ZipWriter};

fn background_package(code: &str, permissions: Value) -> anywhere_plugin::web::VerifiedWebPackage {
    let manifest = json!({"schemaVersion":1,"id":"dev.example.background","name":"Background","version":"1.0.0","apiVersion":"1","platforms":["macos"],"permissions":permissions,
        "background":{"entry":"background.js","resolvers":[{"id":"parse","accepts":["core.text"],"produces":["plugin.dev.example.background.result"],"entrypoint":"parse"}]},
        "commands":[{"id":"copy","title":"Copy parsed value","accepts":["plugin.dev.example.background.result"],"runtime":"background","entrypoint":"copy"}]});
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
        ("background.js", code.as_bytes().to_vec()),
    ] {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    anywhere_plugin::web::VerifiedWebPackage::read(Cursor::new(zip.finish().unwrap().into_inner()))
        .unwrap()
}
fn permissions() -> Value {
    json!([{"name":"context.read","types":["core.text","plugin.dev.example.background.result"]},{"name":"clipboard.writeText"}])
}
fn install(
    app: &Application,
    root: &std::path::Path,
    package: &anywhere_plugin::web::VerifiedWebPackage,
) -> WebPackageStore {
    let file = root.join("source.awplugin");
    package
        .write_archive(std::fs::File::create(&file).unwrap())
        .unwrap();
    let mut store = WebPackageStore::open(root.join("managed"), WebPlatform::Macos, app).unwrap();
    let review = store.inspect(&file).unwrap();
    store.approve(app, &review.token).unwrap();
    store
}

#[tokio::test]
async fn automatic_resolver_cannot_write_and_outputs_remain_package_owned() {
    let code = r#"export async function parse() {
      const result=JSON.parse(__anywhereInvoke('clipboard.writeText', '{"text":"wrong"}'));
      return [{type:'plugin.dev.example.background.result', value:result.error.code}];
    } export function copy() {}"#;
    let app = app();
    let root = tempfile::tempdir().unwrap();
    let package = background_package(code, permissions());
    let mut store = install(&app, root.path(), &package);
    let providers = app.resolver.providers.lock().unwrap().providers().to_vec();
    let provider = providers
        .iter()
        .find(|p| p.descriptor().id.starts_with("web."))
        .unwrap();
    let output = provider
        .resolve(&context("hello"), &ExecutionContext::detached(0, 0))
        .await
        .unwrap();
    let capture = ContextCapturePresentation {
        source: ContextSource::Clipboard,
        status: ContextCaptureStatus::Captured,
        captured_at_ms: 1000,
        clipboard_age_ms: Some(0),
        text: Some("custom-value".into()),
    };
    assert!(app
        .choose_activation_input(&[capture], 1000, false, true, true)
        .is_some());
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].type_id, "plugin.dev.example.background.result");
    assert_eq!(output[0].value.text(), "\"PERMISSION_DENIED\"");
    let registry = app.resolver.base_registry();
    assert!(offers(&registry, "", &[], None).is_empty());
    assert_eq!(offers(&registry, "", &output, None).len(), 1);
    store
        .set_enabled(&app, &package.manifest().id, false)
        .unwrap();
    assert!(provider
        .resolve(&context("hello"), &ExecutionContext::detached(0, 0))
        .await
        .is_err());
    assert!(!app
        .resolver
        .providers
        .lock()
        .unwrap()
        .ids()
        .iter()
        .any(|id| id.starts_with("web.")));
}

#[tokio::test]
async fn undeclared_native_contexts_and_changed_script_bytes_are_rejected() {
    let app = app();
    let root = tempfile::tempdir().unwrap();
    let package = background_package("export function parse(){return [{type:'core.file',value:'/private/secret'}]} export function copy(){}", permissions());
    let _store = install(&app, root.path(), &package);
    let provider = app
        .resolver
        .providers
        .lock()
        .unwrap()
        .providers()
        .iter()
        .find(|p| p.descriptor().id.starts_with("web."))
        .unwrap()
        .clone();
    assert!(provider
        .resolve(&context("hello"), &ExecutionContext::detached(0, 0))
        .await
        .is_err());
    let file = root
        .path()
        .join("managed")
        .join(package.digest())
        .join("assets/background.js");
    std::fs::write(file, "export function parse(){return []}").unwrap();
    assert!(provider
        .resolve(&context("hello"), &ExecutionContext::detached(0, 0))
        .await
        .is_err());
}

#[tokio::test]
async fn background_action_reuses_consent_and_closing_it_cancels_the_wait() {
    let code = r#"export function parse(){return []} export async function copy(){
      const result=JSON.parse(__anywhereInvoke('clipboard.writeText', '{"text":"synthetic"}'));
      if(result.error) throw Error(result.error.code);
    }"#;
    let app = Arc::new(app());
    let root = tempfile::tempdir().unwrap();
    let package = background_package(code, permissions());
    let _store = install(&app, root.path(), &package);
    let session_id = app.begin_activation().unwrap();
    let registry = app.resolver.base_registry();
    let crate::core::action::descriptor::ActionExecutor::WebCommand(command) = &registry
        .get("web:dev.example.background:copy")
        .unwrap()
        .executor
    else {
        panic!("command missing")
    };
    let mut input = ResolvedContext::new(
        "plugin.dev.example.background.result",
        ContextValue::Extension("{}".into()),
        ContextSource::UserInput,
        "test",
    );
    input.session_id = session_id;
    let invocation = start(&mut app.web_plugins.lock().unwrap(), command, &input);
    let binding = app
        .bind_web_document(&invocation, WebPlatform::Macos)
        .unwrap();
    assert!(app.web_document_is_background(&binding).unwrap());
    let work = {
        let app = app.clone();
        let binding = binding.clone();
        tokio::spawn(async move { app.run_background_command(&binding).await })
    };
    let notice = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(notice) = app
                .wait_background_confirmation(&binding, "")
                .await
                .unwrap()
            {
                break notice;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(notice.kind, WebConsentKind::WriteClipboard);
    app.close_web_document(&binding).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), work)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(app
        .confirm_background_operation(&binding, &notice.ticket, true)
        .await
        .is_err());
    assert!(app.web_plugins.lock().unwrap().backgrounds.is_empty());
    // A new tool must release the old job even if native disposal arrives late.
    let invocation = start(&mut app.web_plugins.lock().unwrap(), command, &input);
    let old = app
        .bind_web_document(&invocation, WebPlatform::Macos)
        .unwrap();
    let work = {
        let app = app.clone();
        let old = old.clone();
        tokio::spawn(async move { app.run_background_command(&old).await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while app
            .wait_background_confirmation(&old, "")
            .await
            .unwrap()
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let replacement = start(&mut app.web_plugins.lock().unwrap(), command, &input);
    app.close_web_document(&old).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), work)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    let replacement = app
        .bind_web_document(&replacement, WebPlatform::Macos)
        .unwrap();
    assert!(app.web_document_is_background(&replacement).unwrap());
    app.close_web_document(&replacement).unwrap();
    for approved in [false, true] {
        let invocation = start(&mut app.web_plugins.lock().unwrap(), command, &input);
        let binding = app
            .bind_web_document(&invocation, WebPlatform::Macos)
            .unwrap();
        let work = {
            let app = app.clone();
            let binding = binding.clone();
            tokio::spawn(async move { app.run_background_command(&binding).await })
        };
        let notice = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(notice) = app
                    .wait_background_confirmation(&binding, "")
                    .await
                    .unwrap()
                {
                    break notice;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        app.confirm_background_operation(&binding, &notice.ticket, approved)
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), work)
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            "null"
        );
        app.close_web_document(&binding).unwrap();
    }
}

#[tokio::test]
async fn removing_resolvers_restarts_the_graph_without_publishing_stale_plugin_contexts() {
    let app = app();
    let root = tempfile::tempdir().unwrap();
    let package = background_package("export function parse(){return [{type:'plugin.dev.example.background.result',value:{ok:true}}]} export function copy(){}", permissions());
    let mut store = install(&app, root.path(), &package);
    let id = app.begin_activation().unwrap();
    app.update_context_input(id, "hello".into(), 1, None, InputMode::Content)
        .await
        .unwrap();
    assert!(app
        .session()
        .unwrap()
        .resolution
        .committed_contexts
        .iter()
        .any(|c| c.type_id.starts_with("plugin.")));
    store
        .set_enabled(&app, &package.manifest().id, false)
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let done = {
                let session = app.session().unwrap();
                session.state.revision == 2 && session.state.context_complete
            };
            if done {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!app
        .session()
        .unwrap()
        .resolution
        .committed_contexts
        .iter()
        .any(|c| c.type_id.starts_with("plugin.")));
}

#[tokio::test]
async fn headless_context_reads_reuse_chunked_unicode_transfer() {
    let code = "export function parse(){return []} export function copy(){const reply=JSON.parse(__anywhereInvoke('context.get','{}'));if(reply.error)throw Error(reply.error.code);return reply.value.value.payload.length}";
    let app = app();
    let root = tempfile::tempdir().unwrap();
    let package = background_package(code, permissions());
    let _store = install(&app, root.path(), &package);
    let id = app.begin_activation().unwrap();
    let registry = app.resolver.base_registry();
    let crate::core::action::descriptor::ActionExecutor::WebCommand(command) = &registry
        .get("web:dev.example.background:copy")
        .unwrap()
        .executor
    else {
        panic!("command")
    };
    let mut input = ResolvedContext::new(
        "plugin.dev.example.background.result",
        ContextValue::Extension(json!({"payload":"中🚀".repeat(30000)}).to_string().into()),
        ContextSource::UserInput,
        "test",
    );
    input.session_id = id;
    let invocation = start(&mut app.web_plugins.lock().unwrap(), command, &input);
    let binding = app
        .bind_web_document(&invocation, WebPlatform::Macos)
        .unwrap();
    assert_eq!(app.run_background_command(&binding).await.unwrap(), "90000");
    app.close_web_document(&binding).unwrap();
}
