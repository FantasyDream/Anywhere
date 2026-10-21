use super::{
    tests::{approval, context, package_named, reference, source, start},
    *,
};
use crate::{
    core::{action::descriptor::ActionRegistry, ranking::usage::MemoryUsage},
    persistence::module_storage::ModuleStorageHost,
    Application,
};
use anywhere_protocol::{web_bridge::*, web_plugin::*};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct ClipboardPort {
    reads: AtomicUsize,
    writes: Mutex<Vec<String>>,
    text: Mutex<String>,
    fail: bool,
}
impl anywhere_platform::Platform for ClipboardPort {
    fn frontmost_application(&self) -> anyhow::Result<Option<anywhere_platform::ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(
        &self,
        _: &anywhere_platform::ApplicationInfo,
    ) -> anyhow::Result<Option<PathBuf>> {
        Ok(None)
    }
    fn copy_text(&self, text: &str) -> anyhow::Result<()> {
        self.writes.lock().unwrap().push(text.into());
        Ok(())
    }
    fn clipboard_text(&self) -> anyhow::Result<Option<String>> {
        self.reads.fetch_add(1, Ordering::AcqRel);
        if self.fail {
            anyhow::bail!("private /Users/example/secrets.txt: sensitive failure");
        }
        Ok(Some(self.text.lock().unwrap().clone()))
    }
    fn open_url(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("unexpected operation")
    }
    fn open_file(&self, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("unexpected operation")
    }
    fn reveal_file(&self, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("unexpected operation")
    }
    fn open_directory(&self, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("unexpected operation")
    }
    fn open_terminal(&self, _: &Path) -> anyhow::Result<()> {
        anyhow::bail!("unexpected operation")
    }
}
fn setup(
    permissions: Value,
    port: Arc<ClipboardPort>,
    storage: ModuleStorageHost,
    origin: &str,
) -> (Arc<Application>, WebDocumentBinding) {
    setup_named("dev.example.tool", permissions, port, storage, origin)
}
fn setup_named(
    id: &str,
    permissions: Value,
    port: Arc<ClipboardPort>,
    storage: ModuleStorageHost,
    origin: &str,
) -> (Arc<Application>, WebDocumentBinding) {
    let app = Arc::new(Application::new(
        port,
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    ));
    app.configure_web_storage(storage).unwrap();
    let package = package_named(id, permissions, json!([anywhere_protocol::TEXT]), "fixture");
    let mut grant = approval(&package);
    grant.source_id = origin.into();
    app.publish_web_catalog(
        WebPluginCatalog::prepare(WebPlatform::Macos, [(&package, grant)]).unwrap(),
    )
    .unwrap();
    app.begin_activation().unwrap();
    let registry = app.resolver.base_registry();
    let action = registry.get(&format!("web:{id}:open")).unwrap();
    let crate::core::action::descriptor::ActionExecutor::WebCommand(reference) = &action.executor
    else {
        panic!("wrong fixture action");
    };
    let id = start(
        &mut app.web_plugins.lock().unwrap(),
        reference,
        &context("synthetic input"),
    );
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    (app, binding)
}
fn frame(id: &str, method: &str, params: Value) -> String {
    json!({"protocol":1,"requestId":id,"method":method,"params":params}).to_string()
}
async fn call(
    app: &Application,
    binding: &WebDocumentBinding,
    id: &str,
    method: &str,
    params: Value,
) -> WebDispatch {
    app.dispatch_web_message(binding, source(binding), &frame(id, method, params))
        .await
}
fn reply(result: WebDispatch) -> Value {
    let WebDispatch::Reply(raw) = result else {
        panic!("unexpected confirmation");
    };
    assert!(raw.len() <= MAX_FRAME_BYTES);
    serde_json::from_str(&raw).unwrap()
}
fn confirmation(result: WebDispatch) -> WebConfirmation {
    let WebDispatch::Confirmation(value) = result else {
        panic!("expected native confirmation");
    };
    value
}
fn permissions() -> Value {
    json!([{"name":"clipboard.readText"},{"name":"clipboard.writeText"},{"name":"storage.local"},{"name":"context.read","types":[anywhere_protocol::TEXT]}])
}

#[tokio::test]
async fn typed_permissions_and_native_consent_are_required_and_not_replayable() {
    let port = Arc::new(ClipboardPort {
        text: Mutex::new("synthetic clipboard".into()),
        ..Default::default()
    });
    let (app, binding) = setup(
        permissions(),
        port.clone(),
        ModuleStorageHost::memory().unwrap(),
        "one",
    );
    let read = confirmation(call(&app, &binding, "read", "clipboard.readText", json!({})).await);
    assert_eq!(port.reads.load(Ordering::Acquire), 0);
    assert_eq!(
        reply(call(&app, &binding, "read", "clipboard.readText", json!({})).await)["error"]["code"],
        "DUPLICATE_REQUEST"
    );
    let value = reply(
        app.confirm_web_operation(&binding, source(&binding), &read.ticket, true)
            .await,
    );
    assert_eq!(value["result"]["value"], "synthetic clipboard");
    assert_eq!(port.reads.load(Ordering::Acquire), 1);
    assert!(reply(
        app.confirm_web_operation(&binding, source(&binding), &read.ticket, true)
            .await
    )
    .get("error")
    .is_some());
    assert_eq!(port.reads.load(Ordering::Acquire), 1);
    let copy = confirmation(
        call(
            &app,
            &binding,
            "copy",
            "clipboard.writeText",
            json!({"text":"exact approved text"}),
        )
        .await,
    );
    assert!(port.writes.lock().unwrap().is_empty());
    assert_eq!(
        reply(
            app.confirm_web_operation(&binding, source(&binding), &copy.ticket, false)
                .await
        )["error"]["code"],
        "USER_CANCELLED"
    );
    assert!(port.writes.lock().unwrap().is_empty());
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "spoof",
                "clipboard.readText",
                json!({"userGesture":true})
            )
            .await
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    let (denied, denied_binding) = setup(
        json!([]),
        port.clone(),
        ModuleStorageHost::memory().unwrap(),
        "two",
    );
    assert_eq!(
        reply(
            call(
                &denied,
                &denied_binding,
                "x",
                "clipboard.readText",
                json!({})
            )
            .await
        )["error"]["code"],
        "PERMISSION_DENIED"
    );
    assert_eq!(
        reply(
            call(
                &denied,
                &denied_binding,
                "y",
                "text.begin",
                json!({"bytes":20})
            )
            .await
        )["error"]["code"],
        "PERMISSION_DENIED"
    );
}

#[tokio::test]
async fn persistent_storage_is_scoped_to_plugin_source_and_rejects_namespace_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("web.sqlite3");
    let port = Arc::new(ClipboardPort::default());
    let (app, binding) = setup(
        permissions(),
        port.clone(),
        ModuleStorageHost::open(&path).unwrap(),
        "original",
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "set",
                "storage.set",
                json!({"key":"prefs","value":{"indent":2}})
            )
            .await
        )["result"]["value"],
        Value::Null
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "namespace",
                "storage.get",
                json!({"key":"prefs","namespace":"other"})
            )
            .await
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "key",
                "storage.get",
                json!({"key":"../prefs"})
            )
            .await
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    drop(app);
    let (restarted, binding) = setup(
        permissions(),
        port.clone(),
        ModuleStorageHost::open(&path).unwrap(),
        "original",
    );
    assert_eq!(
        reply(
            call(
                &restarted,
                &binding,
                "get",
                "storage.get",
                json!({"key":"prefs"})
            )
            .await
        )["result"]["value"]["indent"],
        2
    );
    let (other, other_binding) = setup(
        permissions(),
        port,
        ModuleStorageHost::open(&path).unwrap(),
        "different source",
    );
    assert_eq!(
        reply(
            call(
                &other,
                &other_binding,
                "get",
                "storage.get",
                json!({"key":"prefs"})
            )
            .await
        )["result"]["value"],
        Value::Null
    );
    assert!(reply(
        call(
            &restarted,
            &binding,
            "remove",
            "storage.remove",
            json!({"key":"prefs"})
        )
        .await
    )
    .get("error")
    .is_none());
}

#[tokio::test]
async fn storage_quota_failures_preserve_existing_values() {
    let (app, binding) = setup(
        permissions(),
        Arc::new(ClipboardPort::default()),
        ModuleStorageHost::memory().unwrap(),
        "quota",
    );
    for n in 0..17 {
        assert!(reply(
            call(
                &app,
                &binding,
                &format!("r{n}"),
                "storage.set",
                json!({"key":format!("k{n}"),"value":"x".repeat(60000)})
            )
            .await
        )
        .get("error")
        .is_none());
    }
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "full",
                "storage.set",
                json!({"key":"overflow","value":"y".repeat(60000)})
            )
            .await
        )["error"]["code"],
        "QUOTA_EXCEEDED"
    );
    assert_eq!(
        reply(call(&app, &binding, "old", "storage.get", json!({"key":"k0"})).await)["result"]
            ["value"]
            .as_str()
            .unwrap()
            .len(),
        60000
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "missing",
                "storage.get",
                json!({"key":"overflow"})
            )
            .await
        )["result"]["value"],
        Value::Null
    );
}

#[tokio::test]
async fn large_unicode_copy_is_bounded_sequenced_and_only_runs_after_confirmation() {
    let port = Arc::new(ClipboardPort::default());
    let (app, binding) = setup(
        permissions(),
        port.clone(),
        ModuleStorageHost::memory().unwrap(),
        "large",
    );
    let text = "中🚀\u{feff}".repeat(10000);
    let begin = reply(
        call(
            &app,
            &binding,
            "begin",
            "text.begin",
            json!({"bytes":text.len()}),
        )
        .await,
    );
    let ticket = begin["result"]["value"]["ticket"].as_str().unwrap();
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "wrong",
                "text.chunk",
                json!({"ticket":ticket,"sequence":1,"text":"x"})
            )
            .await
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    let mut sequence = 0;
    let mut offset = 0;
    while offset < text.len() {
        let mut end = (offset + MAX_CHUNK_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        assert!(reply(
            call(
                &app,
                &binding,
                &format!("chunk{sequence}"),
                "text.chunk",
                json!({"ticket":ticket,"sequence":sequence,"text":&text[offset..end]})
            )
            .await
        )
        .get("error")
        .is_none());
        sequence += 1;
        offset = end;
    }
    let confirm = confirmation(
        call(
            &app,
            &binding,
            "commit",
            "text.commit",
            json!({"ticket":ticket}),
        )
        .await,
    );
    assert!(port.writes.lock().unwrap().is_empty());
    assert!(reply(
        app.confirm_web_operation(&binding, source(&binding), &confirm.ticket, true)
            .await
    )
    .get("error")
    .is_none());
    assert_eq!(*port.writes.lock().unwrap(), vec![text]);
}

#[tokio::test]
async fn large_input_is_downloaded_in_bounded_utf8_chunks_and_ticket_is_single_use() {
    let (app, _binding) = setup(
        permissions(),
        Arc::new(ClipboardPort::default()),
        ModuleStorageHost::memory().unwrap(),
        "download",
    );
    let text = "🚀中".repeat(20000);
    let reference = reference(&app.resolver.base_registry());
    let id = start(
        &mut app.web_plugins.lock().unwrap(),
        &reference,
        &context(&text),
    );
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    let response = reply(call(&app, &binding, "input", "invocation.get", json!({})).await);
    let ticket = response["result"]["transfer"]["ticket"].as_str().unwrap();
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "wrong",
                "text.read",
                json!({"ticket":ticket,"sequence":1})
            )
            .await
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    let mut data = String::new();
    let mut sequence = 0;
    loop {
        let part = reply(
            call(
                &app,
                &binding,
                &format!("part{sequence}"),
                "text.read",
                json!({"ticket":ticket,"sequence":sequence}),
            )
            .await,
        );
        let value = &part["result"]["value"];
        let chunk = value["text"].as_str().unwrap();
        assert!(chunk.len() <= MAX_CHUNK_BYTES);
        data.push_str(chunk);
        sequence += 1;
        if value["done"] == true {
            break;
        }
    }
    let input: Value = serde_json::from_str(&data).unwrap();
    assert_eq!(input["input"]["text"], text);
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "again",
                "text.read",
                json!({"ticket":ticket,"sequence":sequence})
            )
            .await
        )["error"]["code"],
        "EXPIRED"
    );
    drop(binding);
}

#[tokio::test]
async fn queued_confirmed_operation_is_rechecked_after_hide_and_errors_do_not_leak_paths() {
    let port = Arc::new(ClipboardPort {
        fail: true,
        ..Default::default()
    });
    let (app, binding) = setup(
        permissions(),
        port.clone(),
        ModuleStorageHost::memory().unwrap(),
        "hide",
    );
    let bad = confirmation(call(&app, &binding, "read", "clipboard.readText", json!({})).await);
    let failure = reply(
        app.confirm_web_operation(&binding, source(&binding), &bad.ticket, true)
            .await,
    );
    assert_eq!(failure["error"]["code"], "HOST_FAILURE");
    assert!(!failure.to_string().contains("/Users"));
    assert!(!failure.to_string().contains("sensitive"));
    let pending = confirmation(
        call(
            &app,
            &binding,
            "copy",
            "clipboard.writeText",
            json!({"text":"must not copy"}),
        )
        .await,
    );
    let held = app
        .web_rpc_workers
        .clone()
        .acquire_many_owned(MAX_RUNNING as u32)
        .await
        .unwrap();
    let worker_app = app.clone();
    let worker_binding = binding.clone();
    let confirm = tokio::spawn(async move {
        worker_app
            .confirm_web_operation(
                &worker_binding,
                source(&worker_binding),
                &pending.ticket,
                true,
            )
            .await
    });
    tokio::task::yield_now().await;
    app.suspend_session(1).await.unwrap();
    drop(held);
    assert!(reply(confirm.await.unwrap()).get("error").is_some());
    assert!(port.writes.lock().unwrap().is_empty());
    assert!(
        reply(call(&app, &binding, "stale", "invocation.get", json!({})).await)
            .get("error")
            .is_some()
    );
}

#[tokio::test(start_paused = true)]
async fn text_and_consent_timeouts_release_buffers_and_request_bookkeeping_is_bounded() {
    use super::rpc_state::RpcState;
    let mut state = RpcState::default();
    assert_eq!(
        state.begin_text(MAX_TEXT_BYTES + 1).err().unwrap().code,
        WebErrorCode::TooLarge
    );
    let ticket = state.begin_text(4).unwrap()["ticket"]
        .as_str()
        .unwrap()
        .to_string();
    tokio::time::advance(Duration::from_millis(REQUEST_TIMEOUT_MS)).await;
    assert_eq!(
        state.commit_text(&ticket).err().unwrap().code,
        WebErrorCode::Expired
    );
    for n in 0..MAX_PENDING {
        let request = state.admit(&format!("r{n}"), false, true).unwrap();
        if n < MAX_RUNNING {
            request.control.running.store(true, Ordering::Release);
        }
    }
    assert_eq!(
        state.admit("overflow", false, true).err().unwrap().code,
        WebErrorCode::Busy
    );
    let cancel = state.admit("cancel", false, false).unwrap();
    assert!(state.cancel("r0"));
    state.finish(&cancel);
    assert_eq!(
        state.admit("r0", false, false).err().unwrap().code,
        WebErrorCode::DuplicateRequest
    );
    let old = state.pending["r1"].clone();
    drop(state);
    assert_eq!(old.check().err().unwrap().code, WebErrorCode::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn idle_documents_eagerly_release_expired_transfers_and_consent_without_new_requests() {
    let (app, binding) = setup(
        permissions(),
        Arc::new(ClipboardPort::default()),
        ModuleStorageHost::memory().unwrap(),
        "sweep",
    );
    super::tests::rpc_value(&app, &binding, "ready", "lifecycle.ready").await;
    let id = binding.invocation_id.clone();
    {
        let mut web = app.web_plugins.lock().unwrap();
        web.invocations
            .get_mut(&id)
            .unwrap()
            .rpc
            .begin_text(100)
            .unwrap();
    }
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(6)).await;
    tokio::task::yield_now().await;
    {
        let mut web = app.web_plugins.lock().unwrap();
        let rpc = &mut web.invocations.get_mut(&id).unwrap().rpc;
        assert!(rpc.transfer.is_none());
        let request = rpc.admit("prompt", true, true).unwrap();
        rpc.confirmation = Some(super::rpc_state::Confirmation {
            id: "native-only".into(),
            request,
            operation: super::rpc_state::ClipboardOperation::Read,
        });
    }
    tokio::time::advance(Duration::from_secs(31)).await;
    tokio::task::yield_now().await;
    let web = app.web_plugins.lock().unwrap();
    let rpc = &web.invocations[&id].rpc;
    assert!(rpc.confirmation.is_none());
    assert!(rpc.pending.is_empty());
}

#[tokio::test]
async fn plugin_identity_and_source_both_isolate_storage() {
    let store = ModuleStorageHost::memory().unwrap();
    let port = Arc::new(ClipboardPort::default());
    let (first, a) = setup(permissions(), port.clone(), store.clone(), "same source");
    assert!(reply(
        call(
            &first,
            &a,
            "set",
            "storage.set",
            json!({"key":"private","value":"one"})
        )
        .await
    )
    .get("error")
    .is_none());
    let (second, b) = setup_named(
        "dev.example.other",
        permissions(),
        port,
        store,
        "same source",
    );
    assert_eq!(
        reply(call(&second, &b, "get", "storage.get", json!({"key":"private"})).await)["result"]
            ["value"],
        Value::Null
    );
}

#[test]
fn web_storage_key_limit_does_not_expand_native_module_quotas() {
    use crate::core::storage::StorageQuotaExceeded;
    let host = ModuleStorageHost::memory().unwrap();
    let web = host.bind_with_quota("web.test", 1024 * 1024, 256).unwrap();
    for n in 0..256 {
        web.put(&format!("k{n}"), b"null").unwrap();
    }
    assert!(web
        .put("overflow", b"null")
        .err()
        .unwrap()
        .is::<StorageQuotaExceeded>());
    let native = host.bind("native.test").unwrap();
    for n in 0..128 {
        native.put(&format!("k{n}"), b"null").unwrap();
    }
    assert!(native.put("overflow", b"null").is_err());
}
