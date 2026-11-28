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
    urls: Mutex<Vec<String>>,
    before_open_url: Option<Box<dyn Fn() + Send + Sync>>,
    text: Mutex<String>,
    fail: bool,
    frontmost: Option<anywhere_platform::ApplicationInfo>,
}
impl anywhere_platform::ApplicationCatalog for ClipboardPort {}
impl anywhere_platform::ApplicationHost for ClipboardPort {
    fn frontmost_application(&self) -> anyhow::Result<Option<anywhere_platform::ApplicationInfo>> {
        Ok(self.frontmost.clone())
    }
}
impl anywhere_platform::ContextReader for ClipboardPort {
    fn application_directory(
        &self,
        _: &anywhere_platform::ApplicationInfo,
    ) -> anyhow::Result<Option<PathBuf>> {
        Ok(None)
    }
}
impl anywhere_platform::ClipboardHost for ClipboardPort {
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
}
impl anywhere_platform::FileSearchHost for ClipboardPort {}
impl anywhere_platform::ResourceOpener for ClipboardPort {
    fn open_url(&self, value: &str) -> anyhow::Result<()> {
        if let Some(callback) = &self.before_open_url {
            callback();
        }
        self.urls.lock().unwrap().push(value.into());
        Ok(())
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
impl anywhere_platform::MenuHost for ClipboardPort {}
impl anywhere_platform::GitHost for ClipboardPort {}
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
    let mut input = context("synthetic input");
    input.session_id = app.session().unwrap().state.session_id;
    let id = start(&mut app.web_plugins.lock().unwrap(), reference, &input);
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    (app, binding)
}
fn frame(id: &str, method: &str, params: Value) -> String {
    json!({"protocol":2,"requestId":id,"method":method,"params":params}).to_string()
}
async fn call(
    app: &Application,
    binding: &WebDocumentBinding,
    id: &str,
    method: &str,
    params: Value,
) -> String {
    app.dispatch_web_message(binding, source(binding), &frame(id, method, params))
        .await
}
fn reply(raw: String) -> Value {
    assert!(raw.len() <= MAX_FRAME_BYTES);
    serde_json::from_str(&raw).unwrap()
}
fn permissions() -> Value {
    json!([{"name":"clipboard.readText"},{"name":"clipboard.writeText"},{"name":"storage.local"},{"name":"context.read","types":[anywhere_protocol::TEXT]}])
}

#[tokio::test]
async fn installed_permissions_allow_direct_clipboard_io_and_requests_cannot_replay() {
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
    let read = reply(call(&app, &binding, "read", "clipboard.readText", json!({})).await);
    assert_eq!(read["result"]["value"], "synthetic clipboard");
    assert_eq!(port.reads.load(Ordering::Acquire), 1);
    assert_eq!(
        reply(call(&app, &binding, "read", "clipboard.readText", json!({})).await)["error"]["code"],
        "DUPLICATE_REQUEST"
    );
    assert_eq!(port.reads.load(Ordering::Acquire), 1);
    let copy = reply(
        call(
            &app,
            &binding,
            "copy",
            "clipboard.writeText",
            json!({"text":"exact granted text"}),
        )
        .await,
    );
    assert!(copy.get("error").is_none());
    assert_eq!(*port.writes.lock().unwrap(), ["exact granted text"]);
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
async fn large_unicode_copy_is_bounded_sequenced_and_only_runs_on_commit() {
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
    assert!(port.writes.lock().unwrap().is_empty());
    let committed = reply(
        call(
            &app,
            &binding,
            "commit",
            "text.commit",
            json!({"ticket":ticket}),
        )
        .await,
    );
    assert!(committed.get("error").is_none());
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
async fn queued_clipboard_operation_is_rechecked_after_hide_and_errors_do_not_leak_paths() {
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
    let failure = reply(call(&app, &binding, "read", "clipboard.readText", json!({})).await);
    assert_eq!(failure["error"]["code"], "HOST_FAILURE");
    assert!(!failure.to_string().contains("/Users"));
    assert!(!failure.to_string().contains("sensitive"));
    let held = app
        .web_rpc_workers
        .clone()
        .acquire_many_owned(MAX_RUNNING as u32)
        .await
        .unwrap();
    let worker_app = app.clone();
    let worker_binding = binding.clone();
    let copy = tokio::spawn(async move {
        call(
            &worker_app,
            &worker_binding,
            "copy",
            "clipboard.writeText",
            json!({"text":"must not copy"}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if app.web_plugins.lock().unwrap().invocations[&binding.invocation_id]
                .rpc
                .pending
                .contains_key("copy")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    app.suspend_session(1).await.unwrap();
    drop(held);
    assert!(reply(copy.await.unwrap()).get("error").is_some());
    assert!(port.writes.lock().unwrap().is_empty());
    assert!(
        reply(call(&app, &binding, "stale", "invocation.get", json!({})).await)
            .get("error")
            .is_some()
    );
}

#[tokio::test(start_paused = true)]
async fn text_timeouts_release_buffers_and_request_bookkeeping_is_bounded() {
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
        let request = state.admit(&format!("r{n}"), true).unwrap();
        if n < MAX_RUNNING {
            request.control.running.store(true, Ordering::Release);
        }
    }
    assert_eq!(
        state.admit("overflow", true).err().unwrap().code,
        WebErrorCode::Busy
    );
    let cancel = state.admit("cancel", false).unwrap();
    assert!(state.cancel("r0"));
    state.finish(&cancel);
    assert_eq!(
        state.admit("r0", false).err().unwrap().code,
        WebErrorCode::DuplicateRequest
    );
    let old = state.pending["r1"].clone();
    drop(state);
    assert_eq!(old.check().err().unwrap().code, WebErrorCode::Cancelled);
}

#[tokio::test]
async fn cancelled_queued_requests_release_bookkeeping_without_waiting_for_workers() {
    for hidden in [false, true] {
        let port = Arc::new(ClipboardPort::default());
        let (app, binding) = setup(
            permissions(),
            port.clone(),
            ModuleStorageHost::memory().unwrap(),
            "queued-cancel",
        );
        let held = app
            .web_rpc_workers
            .clone()
            .acquire_many_owned(MAX_RUNNING as u32)
            .await
            .unwrap();
        let queued_app = app.clone();
        let queued_binding = binding.clone();
        let mut queued = tokio::spawn(async move {
            call(
                &queued_app,
                &queued_binding,
                "queued-copy",
                "clipboard.writeText",
                json!({"text":"must not copy"}),
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if app.web_plugins.lock().unwrap().invocations[&binding.invocation_id]
                    .rpc
                    .pending
                    .contains_key("queued-copy")
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        if hidden {
            app.suspend_session(1).await.unwrap();
        } else {
            let cancel = reply(
                call(
                    &app,
                    &binding,
                    "cancel-copy",
                    "request.cancel",
                    json!({"requestId":"queued-copy"}),
                )
                .await,
            );
            assert_eq!(cancel["result"]["value"]["cancelled"], true);
        }
        let completed = tokio::time::timeout(Duration::from_secs(1), &mut queued).await;
        let response = match completed {
            Ok(response) => response.unwrap(),
            Err(_) => {
                drop(held);
                let _ = queued.await;
                panic!("queued cancellation waited for a worker (hidden={hidden})");
            }
        };
        assert_eq!(reply(response)["error"]["code"], "CANCELLED");
        assert!(
            app.web_plugins.lock().unwrap().invocations[&binding.invocation_id]
                .rpc
                .pending
                .is_empty()
        );
        assert!(port.writes.lock().unwrap().is_empty());
        drop(held);
    }
}

#[tokio::test(start_paused = true)]
async fn idle_documents_eagerly_release_expired_transfers_without_new_requests() {
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
        assert!(rpc.pending.is_empty());
    }
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

#[tokio::test]
async fn snapshot_input_and_live_app_require_separate_grants_and_reject_closed_documents() {
    let port = Arc::new(ClipboardPort::default());
    let (app, binding) = setup(
        json!([{"name":"context.read","types":["core.text"]},{"name":"input.read"},{"name":"input.write"},{"name":"context.currentApp"},{"name":"app.frontmost"}]),
        port,
        ModuleStorageHost::memory().unwrap(),
        "state",
    );
    let value = reply(call(&app, &binding, "state", "state.get", json!({})).await);
    assert_eq!(
        value["result"]["value"]["context"]["text"],
        "synthetic input"
    );
    assert_eq!(value["result"]["value"]["input"], "");
    assert!(
        reply(call(&app, &binding, "source", "app.getSource", json!({})).await)["result"]["value"]
            .is_null()
    );
    assert!(
        reply(call(&app, &binding, "live", "app.getFrontmost", json!({})).await)["result"]["value"]
            .is_null()
    );
    assert!(reply(
        call(
            &app,
            &binding,
            "write",
            "input.set",
            json!({"text":"hello"})
        )
        .await
    )
    .get("error")
    .is_none());
    assert_eq!(app.session().unwrap().state.input, "hello");
    let event = app.wait_web_state(&binding, 0).await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&event).unwrap()["state"]["input"],
        "hello"
    );
    let pending = {
        let app = app.clone();
        let binding = binding.clone();
        tokio::spawn(async move { app.wait_web_state(&binding, u64::MAX).await })
    };
    tokio::task::yield_now().await;
    app.close_web_document(&binding).unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(250), pending)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(app.wait_web_state(&binding, 0).await.is_err());
    assert!(
        reply(call(&app, &binding, "late", "input.set", json!({"text":"wrong"})).await)
            .get("error")
            .is_some()
    );
    assert_eq!(app.session().unwrap().state.input, "hello");
    let (app, binding) = setup(
        json!([]),
        Arc::new(ClipboardPort::default()),
        ModuleStorageHost::memory().unwrap(),
        "denied",
    );
    for method in ["input.get", "app.getSource", "app.getFrontmost"] {
        assert_eq!(
            reply(
                call(
                    &app,
                    &binding,
                    &format!("denied_{}", method.replace('.', "_")),
                    method,
                    json!({})
                )
                .await
            )["error"]["code"],
            "PERMISSION_DENIED"
        );
    }
    let value = reply(call(&app, &binding, "state", "state.get", json!({})).await);
    assert!(value["result"]["value"]["input"].is_null());
    assert_eq!(
        value["result"]["value"]["context"]["reason"],
        "permission_denied"
    );
}

#[tokio::test]
async fn source_application_is_frozen_while_frontmost_is_queried_live() {
    let live = anywhere_platform::ApplicationInfo {
        bundle_id: "app.live".into(),
        name: "Live".into(),
        pid: 2,
        instance_id: None,
    };
    let frozen = anywhere_platform::ApplicationInfo {
        bundle_id: "app.source".into(),
        name: "Source".into(),
        pid: 1,
        instance_id: None,
    };
    let (app, old) = setup(
        json!([{"name":"context.currentApp"},{"name":"app.frontmost"}]),
        Arc::new(ClipboardPort {
            frontmost: Some(live),
            ..Default::default()
        }),
        ModuleStorageHost::memory().unwrap(),
        "apps",
    );
    app.close_web_document(&old).unwrap();
    let registry = app.resolver.base_registry();
    let crate::core::action::descriptor::ActionExecutor::WebCommand(command) =
        &registry.get("web:dev.example.tool:open").unwrap().executor
    else {
        panic!("command")
    };
    let mut input = context("text");
    input.session_id = app.session().unwrap().state.session_id;
    let anywhere_protocol::HostView::WebPlugin { invocation_id } = app
        .web_plugins
        .lock()
        .unwrap()
        .start(
            command,
            &input,
            input.session_id,
            input.revision,
            Some(&frozen),
            &frozen.bundle_id,
        )
        .unwrap()
    else {
        panic!("view")
    };
    let binding = app
        .bind_web_document(&invocation_id, WebPlatform::Macos)
        .unwrap();
    assert_eq!(
        reply(call(&app, &binding, "source", "app.getSource", json!({})).await)["result"]["value"]
            ["id"],
        "app.source"
    );
    assert_eq!(
        reply(call(&app, &binding, "live", "app.getFrontmost", json!({})).await)["result"]["value"]
            ["id"],
        "app.live"
    );
}

#[tokio::test]
async fn url_confirmation_uses_canonical_origin_and_rejects_busy_or_revoked_requests() {
    let port = Arc::new(ClipboardPort::default());
    let (app, binding) = setup(
        json!([{"name":"url.open","origins":["https://例え.example"]}]),
        port.clone(),
        ModuleStorageHost::memory().unwrap(),
        "one",
    );
    let mut interactions = app.web_host_interactions().unwrap();
    let make_call = |id: &'static str| {
        let app = app.clone();
        let binding = binding.clone();
        tokio::spawn(async move {
            reply(
                call(
                    &app,
                    &binding,
                    id,
                    "url.open",
                    json!({"url":"https://例え.example/a?q=1"}),
                )
                .await,
            )
        })
    };
    let outside = reply(
        call(
            &app,
            &binding,
            "outside",
            "url.open",
            json!({"url":"https://elsewhere.example"}),
        )
        .await,
    );
    assert_eq!(outside["error"]["code"], "PERMISSION_DENIED");
    let first = make_call("first");
    let consent = interactions.recv().await.unwrap();
    assert_eq!(app.web_rpc_workers.available_permits(), MAX_RUNNING);
    let HostInteractionKind::OpenUrl {
        origin,
        destination,
        query_bytes,
    } = &consent.kind
    else {
        panic!("expected URL consent");
    };
    assert_eq!(origin, "https://xn--r8jz45g.example");
    assert_eq!(destination, "https://xn--r8jz45g.example/a?q=1");
    assert_eq!(*query_bytes, 3);
    assert_eq!(make_call("busy").await.unwrap()["error"]["code"], "BUSY");
    consent.respond(Err(WebErrorCode::UserCancelled));
    assert_eq!(first.await.unwrap()["error"]["code"], "USER_CANCELLED");
    let second = make_call("approved");
    interactions.recv().await.unwrap().respond(Ok(()));
    assert!(second.await.unwrap().get("result").is_some());
    assert_eq!(
        *port.urls.lock().unwrap(),
        ["https://xn--r8jz45g.example/a?q=1"]
    );
    let revoked = make_call("revoked");
    let consent = interactions.recv().await.unwrap();
    let _new = app
        .bind_web_document(&binding.invocation_id, WebPlatform::Macos)
        .unwrap();
    consent.respond(Ok(()));
    assert!(revoked.await.unwrap().get("error").is_some());
    assert_eq!(port.urls.lock().unwrap().len(), 1);
    let outcome = app
        .dispatch_web_outcome(
            &binding,
            source(&binding),
            &frame("close", "window.close", json!({})),
        )
        .await;
    assert!(outcome.host_action.is_none());
}

#[cfg(any(target_os = "macos", windows))]
#[tokio::test]
async fn file_capabilities_are_scoped_to_the_captured_identity_and_document() {
    use crate::{application::context_capture::finder_contexts, core::context::ContextValue};
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().canonicalize().unwrap();
    let path = dir.join("选区.csv");
    std::fs::write(&path, "a,b\n1,2").unwrap();
    let port = Arc::new(ClipboardPort::default());
    let app = Arc::new(Application::new(
        port,
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    ));
    let package = package_named(
        "dev.example.tool",
        json!([
            {"name":"context.read","types":["core.file"]},{"name":"resources.read"},{"name":"preview.open"}
        ]),
        json!(["core.file"]),
        "fixture",
    );
    app.publish_web_catalog(
        WebPluginCatalog::prepare(WebPlatform::Macos, [(&package, approval(&package))]).unwrap(),
    )
    .unwrap();
    let session = app.begin_activation().unwrap();
    let registry = app.resolver.base_registry();
    let mut contexts = finder_contexts(vec![path.clone()]).unwrap();
    let mut selected = contexts.pop().unwrap();
    assert!(matches!(selected.value, ContextValue::File(_)));
    selected.session_id = session;
    let id = start(
        &mut app.web_plugins.lock().unwrap(),
        &reference(&registry),
        &selected,
    );
    let binding = app.bind_web_document(&id, WebPlatform::Macos).unwrap();
    let invocation = app.web_invocation(&binding, source(&binding)).unwrap();
    let WebInput::Resources { items, .. } = invocation.input else {
        panic!("selected file missing");
    };
    let item = &items[0];
    assert!(item.readable && item.previewable);
    let params = json!({"handle":item.handle});
    let stat = reply(call(&app, &binding, "stat", "resources.stat", params.clone()).await);
    assert_eq!(stat["result"]["value"]["bytes"], 7);
    assert_eq!(stat["result"]["value"]["name"], "选区.csv");
    assert_eq!(
        reply(call(&app, &binding, "read", "resources.readText", params.clone()).await)["result"]
            ["value"],
        "a,b\n1,2"
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "forged",
                "resources.readText",
                json!({"handle":"forged"})
            )
            .await
        )["error"]["code"],
        "EXPIRED"
    );
    assert_eq!(
        reply(
            call(
                &app,
                &binding,
                "small",
                "resources.readText",
                json!({"handle":item.handle,"maxBytes":6})
            )
            .await
        )["error"]["code"],
        "TOO_LARGE"
    );
    let mut interactions = app.web_host_interactions().unwrap();
    let preview = {
        let app = app.clone();
        let binding = binding.clone();
        let params = params.clone();
        tokio::spawn(
            async move { reply(call(&app, &binding, "preview", "preview.open", params).await) },
        )
    };
    let offer = interactions.recv().await.unwrap();
    let HostInteractionKind::Preview(resource) = &offer.kind else {
        panic!("missing native preview")
    };
    let snapshot = PathBuf::from(&resource.paths[0]);
    let preview_id = resource.id.clone();
    assert_ne!(snapshot, path);
    assert_eq!(snapshot.extension().unwrap(), "csv");
    assert!(snapshot.metadata().unwrap().permissions().readonly());
    assert_eq!(std::fs::read_to_string(&snapshot).unwrap(), "a,b\n1,2");
    offer.respond(Ok(()));
    assert!(preview.await.unwrap().get("result").is_some());
    app.release_preview(&preview_id).unwrap();
    assert!(!snapshot.exists());

    for (case, data, expected) in [
        ("empty", vec![], None),
        ("non-utf8", vec![0xff], Some("INVALID_REQUEST")),
        ("nul", vec![0], Some("INVALID_REQUEST")),
        (
            "over-limit",
            vec![b'x'; MAX_TEXT_BYTES + 1],
            Some("TOO_LARGE"),
        ),
        (
            "bridge-overflow",
            vec![b'"'; MAX_TEXT_BYTES],
            Some("TOO_LARGE"),
        ),
    ] {
        std::fs::write(&path, data).unwrap();
        let response =
            reply(call(&app, &binding, case, "resources.readText", params.clone()).await);
        assert_eq!(response["error"]["code"].as_str(), expected, "{case}");
    }
    std::fs::write(&path, "a,b\n1,2").unwrap();
    let prior = app
        .web_plugins
        .lock()
        .unwrap()
        .tool_window(&binding, session, 0)
        .unwrap();
    let (mut tool, tool_id) = prior;
    let tool_binding = tool.bind(&tool_id, WebPlatform::Macos).unwrap();
    let WebInput::Resources { items, .. } = tool
        .invocation(&tool_binding, source(&tool_binding))
        .unwrap()
        .input
    else {
        panic!("tool file missing")
    };
    assert_ne!(items[0].handle, item.handle);
    app.web_plugins.lock().unwrap().suspend();
    let resumed = app
        .resume_web_document(&binding, WebPlatform::Macos)
        .unwrap();
    assert_eq!(
        reply(
            call(
                &app,
                &resumed,
                "restored",
                "resources.readText",
                params.clone()
            )
            .await
        )["result"]["value"],
        "a,b\n1,2"
    );
    std::fs::rename(&path, dir.join("previous.csv")).unwrap();
    std::fs::write(&path, "replacement").unwrap();
    assert_eq!(
        reply(call(&app, &resumed, "replaced", "resources.readText", params).await)["error"]
            ["code"],
        "EXPIRED"
    );
}

#[tokio::test]
async fn background_handoff_survives_focus_loss_and_denies_new_calls() {
    use super::background_tests::{background_package, install};
    use crate::application::LauncherApplication;
    let opened = Arc::new(tokio::sync::Notify::new());
    let (resume_open, wait_open) = std::sync::mpsc::channel();
    let halt = Mutex::new(wait_open);
    let port = Arc::new(ClipboardPort {
        before_open_url: Some(Box::new({
            let opened = opened.clone();
            move || {
                opened.notify_one();
                halt.lock().unwrap().recv().unwrap();
            }
        })),
        ..Default::default()
    });
    let app = Arc::new(Application::new(
        port.clone(),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    ));
    let root = tempfile::tempdir().unwrap();
    let package = background_package(
        r#"
      export function parse() {
        return [{typeId:'plugin.dev.example.background.result',value:'support'}];
      }
      export function copy(){
        const url = JSON.parse(__anywhereInvoke('url.open', '{"url":"https://example.com/help"}'));
        if(url.error) throw Error(url.error.code);
        const after = JSON.parse(__anywhereInvoke('clipboard.writeText', '{"text":"must fail"}'));
        if(after.error?.code !== 'INACTIVE') throw Error('new call was allowed after focus loss');
        return {message:'Opened'};
      }"#,
        json!([
            {"name":"context.read","types":["core.text","plugin.dev.example.background.result"]},
            {"name":"clipboard.writeText"},{"name":"url.open","origins":["https://example.com"]}
        ]),
    );
    let _store = install(&app, root.path(), &package);
    let mut events = app.web_host_interactions().unwrap();
    let session = app.begin_activation().unwrap();
    let state = app
        .update_context_input(
            session,
            "fixture".into(),
            1,
            None,
            anywhere_protocol::InputMode::Content,
        )
        .await
        .unwrap()
        .unwrap();
    let action = state
        .candidates
        .iter()
        .find(|candidate| {
            candidate.command_id.as_deref() == Some("web:dev.example.background:copy")
        })
        .unwrap()
        .id
        .clone();
    let execution = tokio::spawn({
        let app = app.clone();
        async move { app.execute(session, 1, action).await.unwrap() }
    });
    events.recv().await.unwrap().respond(Ok(()));
    opened.notified().await;
    app.suspend_session_now(session).unwrap();
    resume_open.send(()).unwrap();
    let result = execution.await.unwrap();
    assert!(result.succeeded(), "{:?}", result.error);
    assert_eq!(result.output_text.as_deref(), Some("Opened"));
    assert!(port.writes.lock().unwrap().is_empty());
    assert_eq!(
        app.resolver
            .history
            .lock()
            .unwrap()
            .load_global()
            .unwrap()
            .len(),
        1
    );
}
