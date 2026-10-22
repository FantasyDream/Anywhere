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
