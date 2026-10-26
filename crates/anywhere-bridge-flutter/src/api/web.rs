//! Trusted Flutter/native adapter. None of these Host management operations are JS RPCs.
use super::*;
use anywhere_core::{
    application::web_plugins::{
        WebDispatch, WebDocumentBinding, WebMessageSource, WebPackageReview, WebPackageStatus,
        WebPackageStore,
    },
    persistence::module_storage::ModuleStorageHost,
};
use anywhere_protocol::web_plugin::WebPlatform;
static STORE: OnceLock<Mutex<WebPackageStore>> = OnceLock::new();
static DOCUMENT: Mutex<Option<WebDocumentBinding>> = Mutex::new(None);
static OPEN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn platform() -> Result<WebPlatform> {
    #[cfg(target_os = "macos")]
    {
        Ok(WebPlatform::Macos)
    }
    // Native Windows resource origin and input/security gates have not been accepted.
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("Web tools are not enabled on this platform yet")
    }
}
pub(super) fn initialize(app: &Application, data: &Path) -> Result<()> {
    app.configure_web_storage(ModuleStorageHost::open(&data.join("web-storage.sqlite3"))?)?;
    #[cfg(target_os = "macos")]
    STORE
        .set(Mutex::new(WebPackageStore::open(
            data.join("web-packages"),
            platform()?,
            app,
        )?))
        .map_err(|_| anyhow::anyhow!("Web packages already initialized"))?;
    Ok(())
}
fn store() -> Result<std::sync::MutexGuard<'static, WebPackageStore>> {
    STORE
        .get()
        .context("Web tools are unavailable")?
        .lock()
        .map_err(|_| anyhow::anyhow!("Web package store unavailable"))
}
fn document(handle: &str) -> Result<WebDocumentBinding> {
    DOCUMENT
        .lock()
        .map_err(|_| anyhow::anyhow!("Web document unavailable"))?
        .as_ref()
        .filter(|b| b.runtime_id() == handle)
        .cloned()
        .context("Web document expired")
}
fn source<'a>(
    binding: &'a WebDocumentBinding,
    origin: &'a str,
    main_frame: bool,
) -> WebMessageSource<'a> {
    WebMessageSource {
        runtime_id: binding.runtime_id(),
        document_epoch: binding.document_epoch(),
        origin,
        main_frame,
    }
}
pub(super) fn statuses() -> Result<Vec<WebPackageDto>> {
    Ok(store()?.statuses().into_iter().map(status).collect())
}
fn status(s: WebPackageStatus) -> WebPackageDto {
    WebPackageDto {
        id: s.id,
        name: s.name,
        version: s.version,
        digest: s.digest,
        permissions: permissions(&s.permissions),
        enabled: s.enabled,
        rollback: s.rollback,
    }
}
fn permissions(p: &[anywhere_protocol::web_plugin::WebPermission]) -> Vec<WebPermissionDto> {
    use anywhere_protocol::web_plugin::WebPermission::*;
    p.iter()
        .map(|permission| WebPermissionDto {
            kind: match permission {
                ContextRead { .. } => WebPermissionKindDto::ContextRead,
                CurrentApp {} => WebPermissionKindDto::CurrentApp,
                ClipboardReadText {} => WebPermissionKindDto::ClipboardReadText,
                ClipboardWriteText {} => WebPermissionKindDto::ClipboardWriteText,
                LocalStorage {} => WebPermissionKindDto::LocalStorage,
                InputRead {} => WebPermissionKindDto::InputRead,
                InputWrite {} => WebPermissionKindDto::InputWrite,
                FrontmostApp {} => WebPermissionKindDto::FrontmostApp,
            },
            context_types: match permission {
                ContextRead { types } => types.clone(),
                _ => Vec::new(),
            },
        })
        .collect()
}
pub(super) fn inspect(path: String) -> Result<WebReviewDto> {
    let WebPackageReview {
        token,
        id,
        name,
        version,
        digest,
        permissions: p,
        updating,
    } = store()?.inspect(Path::new(&path))?;
    Ok(WebReviewDto {
        token,
        id,
        name,
        version,
        digest,
        permissions: permissions(&p),
        updating,
    })
}
pub(super) fn cancel_review() -> Result<()> {
    store()?.cancel_review();
    Ok(())
}
pub(super) fn approve(token: String) -> Result<Vec<WebPackageDto>> {
    store()?.approve(application()?, &token)?;
    statuses()
}
pub(super) fn change(id: String, operation: String) -> Result<Vec<WebPackageDto>> {
    {
        let mut store = store()?;
        let app = application()?;
        match operation.as_str() {
            "enable" => store.set_enabled(app, &id, true)?,
            "disable" | "revoke" => store.set_enabled(app, &id, false)?,
            "uninstall" => store.uninstall(app, &id)?,
            "rollback" => store.rollback(app, &id)?,
            _ => anyhow::bail!("unknown Web package operation"),
        }
    }
    statuses()
}
pub(super) async fn open(
    invocation_id: String,
    resume_handle: Option<String>,
) -> Result<WebDocumentDto> {
    // Serialize document attachment/publication, so an older open cannot replace
    // the adapter slot after a newer native page has already received its handle.
    let _opening = OPEN.lock().await;
    let app = application()?;
    let binding = match resume_handle {
        Some(handle) => app.resume_web_document(&document(&handle)?, platform()?)?,
        None => app.bind_web_document(&invocation_id, platform()?)?,
    };
    let invocation = app.web_invocation(&binding, source(&binding, binding.origin(), true))?;
    let installed = store()?
        .statuses()
        .into_iter()
        .find(|p| p.id == invocation.plugin_id && p.enabled && p.digest == binding.content_digest())
        .context("Web package unavailable")?;
    let background = app.web_document_is_background(&binding)?;
    let result = WebDocumentDto {
        background,
        handle: binding.runtime_id().into(),
        origin: binding.origin().into(),
        entry: installed.entry,
        title: installed.name,
    };
    *DOCUMENT
        .lock()
        .map_err(|_| anyhow::anyhow!("Web document unavailable"))? = Some(binding);
    Ok(result)
}
pub(super) fn close(handle: String) -> Result<()> {
    let binding = document(&handle)?;
    application()?.close_web_document(&binding)?;
    let mut current = DOCUMENT
        .lock()
        .map_err(|_| anyhow::anyhow!("Web document unavailable"))?;
    if current.as_ref().is_some_and(|b| b.runtime_id() == handle) {
        *current = None;
    }
    Ok(())
}
pub(super) async fn dispatch(
    handle: String,
    origin: String,
    main_frame: bool,
    frame: String,
) -> Result<WebReplyDto> {
    let binding = document(&handle)?;
    Ok(reply(
        application()?
            .dispatch_web_message(&binding, source(&binding, &origin, main_frame), &frame)
            .await,
    ))
}
pub(super) async fn confirm(
    handle: String,
    origin: String,
    ticket: String,
    approved: bool,
) -> Result<WebReplyDto> {
    let binding = document(&handle)?;
    Ok(reply(
        application()?
            .confirm_web_operation(&binding, source(&binding, &origin, true), &ticket, approved)
            .await,
    ))
}
fn reply(dispatch: WebDispatch) -> WebReplyDto {
    match dispatch {
        WebDispatch::Reply(frame) => WebReplyDto {
            frame: Some(frame),
            ticket: None,
            consent: None,
            preview: None,
        },
        WebDispatch::Confirmation(c) => WebReplyDto {
            frame: None,
            ticket: Some(c.ticket),
            consent: Some(match c.kind {
                anywhere_core::application::web_plugins::WebConsentKind::ReadClipboard => {
                    WebConsentKindDto::ReadClipboard
                }
                anywhere_core::application::web_plugins::WebConsentKind::WriteClipboard => {
                    WebConsentKindDto::WriteClipboard
                }
            }),
            preview: c.preview,
        },
    }
}
pub(super) fn resource(handle: String, path: String) -> Result<Vec<u8>> {
    let binding = document(&handle)?;
    let app = application()?;
    app.validate_web_document(&binding, source(&binding, binding.origin(), true))?;
    let file = store()?.resource_file(binding.content_digest(), &path)?;
    let bytes = file.read()?;
    app.validate_web_document(&binding, source(&binding, binding.origin(), true))?;
    Ok(bytes)
}

pub(super) async fn wait_state(handle: String, after: u64) -> Result<Option<String>> {
    let binding = document(&handle)?;
    application()?
        .wait_web_state(&binding, after)
        .await
        .map_err(|_| anyhow::anyhow!("Plugin state unavailable"))
}

pub(super) async fn run_background(handle: String) -> Result<String> {
    application()?
        .run_background_command(&document(&handle)?)
        .await
}
pub(super) async fn wait_background_confirmation(
    handle: String,
    after: String,
) -> Result<Option<WebReplyDto>> {
    Ok(application()?
        .wait_background_confirmation(&document(&handle)?, &after)
        .await?
        .map(|notice| reply(WebDispatch::Confirmation(notice))))
}
pub(super) async fn confirm_background(
    handle: String,
    ticket: String,
    approved: bool,
) -> Result<()> {
    application()?
        .confirm_background_operation(&document(&handle)?, &ticket, approved)
        .await
}
