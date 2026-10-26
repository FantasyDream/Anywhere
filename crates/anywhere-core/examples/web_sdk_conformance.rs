//! Development-only stdin harness for SDK/Rust conformance. No real OS clipboard or files.
use anyhow::{Context, Result};
use anywhere_core::{
    application::web_plugins::*,
    core::{action::descriptor::ActionRegistry, ranking::usage::MemoryUsage},
    persistence::module_storage::ModuleStorageHost,
    Application, LauncherApplication,
};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::{web_plugin::WebPlatform, HostView};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

struct SyntheticPlatform(Mutex<String>);
impl Platform for SyntheticPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        *self.0.lock().unwrap() = text.into();
        Ok(())
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        Ok(Some(self.0.lock().unwrap().clone()))
    }
    fn open_url(&self, _: &str) -> Result<()> {
        anyhow::bail!("not available in conformance harness")
    }
    fn open_file(&self, _: &Path) -> Result<()> {
        anyhow::bail!("not available in conformance harness")
    }
    fn reveal_file(&self, _: &Path) -> Result<()> {
        anyhow::bail!("not available in conformance harness")
    }
    fn open_directory(&self, _: &Path) -> Result<()> {
        anyhow::bail!("not available in conformance harness")
    }
    fn open_terminal(&self, _: &Path) -> Result<()> {
        anyhow::bail!("not available in conformance harness")
    }
}
fn source(binding: &WebDocumentBinding) -> WebMessageSource<'_> {
    WebMessageSource {
        runtime_id: binding.runtime_id(),
        document_epoch: binding.document_epoch(),
        origin: binding.origin(),
        main_frame: true,
    }
}
fn package() -> Result<anywhere_plugin::web::VerifiedWebPackage> {
    let manifest = json!({"schemaVersion":1,"id":"dev.example.conformance","name":"Conformance","version":"1.0.0","apiVersion":"1","platforms":["macos"],"entry":"index.html","permissions":[{"name":"clipboard.readText"},{"name":"clipboard.writeText"},{"name":"storage.local"},{"name":"context.currentApp"}],"commands":[{"id":"open","title":"Conformance","entrypoint":"workspace"}]});
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, bytes) in [
        ("manifest.json", serde_json::to_vec(&manifest)?),
        ("index.html", b"<!doctype html>fixture".to_vec()),
    ] {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )?;
        zip.write_all(&bytes)?;
    }
    anywhere_plugin::web::VerifiedWebPackage::read(Cursor::new(zip.finish()?.into_inner()))
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let [mode, path] = args.as_slice() {
        anyhow::ensure!(mode == "--background", "unknown harness mode");
        return check_background(Path::new(path)).await;
    }
    anyhow::ensure!(
        args.is_empty(),
        "expected --background PACKAGE or no arguments"
    );
    let app = Application::new(
        Arc::new(SyntheticPlatform(Mutex::new("synthetic clipboard".into()))),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    );
    app.configure_web_storage(ModuleStorageHost::memory()?)?;
    let package = package()?;
    let approval = WebPackageApproval {
        plugin_id: package.manifest().id.clone(),
        content_digest: package.digest().into(),
        permissions: package.manifest().permissions.clone(),
        source_id: "synthetic-test-only".into(),
    };
    app.publish_web_catalog(WebPluginCatalog::prepare(
        WebPlatform::Macos,
        [(&package, approval)],
    )?)?;
    let state = app.activate().await?.context("activation failed")?;
    let state = app
        .update_input(state.session_id, "Conformance".into(), 1)
        .await?
        .context("query failed")?;
    let candidate = state
        .candidates
        .iter()
        .find(|c| c.command_id.as_deref() == Some("web:dev.example.conformance:open"))
        .context("missing fixture action")?;
    let result = app
        .execute(state.session_id, 1, candidate.id.clone())
        .await?;
    let Some(HostView::WebPlugin { invocation_id }) = result.view else {
        anyhow::bail!("fixture invocation failed");
    };
    let mut binding = app.bind_web_document(&invocation_id, WebPlatform::Macos)?;
    println!("{}", json!({"kind":"ready"}));
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let input: Value = serde_json::from_str(&line)?;
        let response = match input["kind"].as_str() {
            Some("call") => {
                app.dispatch_web_message(
                    &binding,
                    source(&binding),
                    input["frame"].as_str().context("missing frame")?,
                )
                .await
            }
            Some("confirm") => {
                app.confirm_web_operation(
                    &binding,
                    source(&binding),
                    input["ticket"].as_str().context("missing confirmation")?,
                    input["approved"] == true,
                )
                .await
            }
            Some("reload") => {
                binding = app.bind_web_document(&invocation_id, WebPlatform::Macos)?;
                println!("{}", json!({"kind":"reloaded"}));
                std::io::stdout().flush()?;
                continue;
            }
            _ => anyhow::bail!("unknown test harness command"),
        };
        let output = match response {
            WebDispatch::Reply(frame) => json!({"kind":"reply","frame":frame}),
            WebDispatch::Confirmation(value) => {
                json!({"kind":"confirmation","ticket":value.ticket,"requestId":value.request_id,"operation":format!("{:?}", value.kind),"preview":value.preview})
            }
        };
        println!("{output}");
        std::io::stdout().flush()?;
    }
    Ok(())
}

/// Execute the built TypeScript bundle through real package approval, Context,
/// Action qualification, QuickJS, the SDK adapter and the existing consent broker.
async fn check_background(path: &Path) -> Result<()> {
    let platform = Arc::new(SyntheticPlatform(Mutex::new(String::new())));
    let app = Arc::new(Application::new(
        platform.clone(),
        ActionRegistry::default(),
        Box::<MemoryUsage>::default(),
        vec![],
    ));
    let root = tempfile::tempdir()?;
    let mut store = WebPackageStore::open(root.path().join("plugins"), WebPlatform::Macos, &app)?;
    let review = store.inspect(path)?;
    let plugin_id = review.id.clone();
    store.approve(&app, &review.token)?;
    let id = app.begin_activation()?;
    let state = app
        .update_context_input(
            id,
            r#"{"one":1,"two":2}"#.into(),
            1,
            None,
            anywhere_protocol::InputMode::Content,
        )
        .await?
        .context("background Context did not publish")?;
    let candidate = state
        .candidates
        .iter()
        .find(|candidate| {
            candidate.command_id.as_deref() == Some(&format!("web:{plugin_id}:copy-summary"))
        })
        .context("background action did not match derived Context")?;
    let output = app.execute(id, 1, candidate.id.clone()).await?;
    let Some(HostView::WebPlugin { invocation_id }) = output.view else {
        anyhow::bail!("missing background invocation")
    };
    let binding = app.bind_web_document(&invocation_id, WebPlatform::Macos)?;
    anyhow::ensure!(
        app.web_document_is_background(&binding)?,
        "unexpected UI command"
    );
    let job = {
        let app = app.clone();
        let binding = binding.clone();
        tokio::spawn(async move { app.run_background_command(&binding).await })
    };
    let notice = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(notice) = app.wait_background_confirmation(&binding, "").await? {
                return Ok::<_, anyhow::Error>(notice);
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    anyhow::ensure!(
        platform.0.lock().unwrap().is_empty(),
        "clipboard changed before consent"
    );
    app.confirm_background_operation(&binding, &notice.ticket, true)
        .await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), job).await???;
    let copied: Value = serde_json::from_str(&platform.0.lock().unwrap())?;
    anyhow::ensure!(
        copied == json!({"kind":"object", "count":2}),
        "SDK result did not survive the complete pipeline"
    );
    app.close_web_document(&binding)?;
    println!("TypeScript background SDK conformance passed (synthetic clipboard only)");
    Ok(())
}
