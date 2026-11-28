//! Frontend-neutral composition and state that is shared by native UI hosts.
//!
//! Frontend bridges may convert these domain values to their own transport DTOs,
//! but business policy, storage ownership and document lifecycles stay here.

use crate::{
    application::{
        builtin_registry,
        settings_service::SettingsService,
        web_plugins::{
            WebDispatchOutcome, WebDocumentBinding, WebHostAction, WebMessageSource,
            WebPackageReview, WebPackageStatus, WebPackageStore,
        },
        Application,
    },
    persistence::usage::SqliteUsageRepository,
};
use anyhow::{Context, Result};
use anywhere_protocol::web_plugin::WebPlatform;
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

/// Filesystem roots owned by one frontend process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    pub data_directory: PathBuf,
    pub bundled_plugin_directory: PathBuf,
}

impl RuntimePaths {
    pub fn new(
        data_directory: impl Into<PathBuf>,
        bundled_plugin_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            data_directory: data_directory.into(),
            bundled_plugin_directory: bundled_plugin_directory.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeErrorKind {
    InvalidRequest,
    Unavailable,
    Stale,
    NotAllowed,
    NotFound,
    Busy,
    Io,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    pub kind: RuntimeErrorKind,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeError {
    pub fn new(kind: RuntimeErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
        }
    }

    pub fn internal(error: impl fmt::Display) -> Self {
        Self::new(
            RuntimeErrorKind::Internal,
            "runtime.internal",
            error.to_string(),
        )
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebPackageOperation {
    Enable,
    Disable,
    Uninstall,
    Rollback,
}

/// Metadata needed by a native web surface after a document has been bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebDocumentView {
    pub binding: WebDocumentBinding,
    pub origin: String,
    pub entry: String,
    pub title: String,
    pub builtin: bool,
}

/// Owns the services shared by the product frontend in one process.
///
/// `SettingsService` and `WebPackageStore` are synchronous. Their standard
/// mutexes are never retained across an await point. Web document open/resume
/// has a separate asynchronous mutex, while dispatch clones the document
/// binding before awaiting application work.
pub struct ApplicationRuntime {
    application: Arc<Application>,
    settings: Arc<Mutex<SettingsService>>,
    web_packages: Arc<Mutex<Option<WebPackageStore>>>,
    web_platform: Option<WebPlatform>,
    document: Mutex<Option<WebDocumentBinding>>,
    opening: tokio::sync::Mutex<()>,
    paths: RuntimePaths,
}

fn resolve_web_platform(platform: &dyn anywhere_platform::Platform) -> Option<WebPlatform> {
    match platform.web_platform_id() {
        Some("macos") => Some(WebPlatform::Macos),
        Some("windows") => Some(WebPlatform::Windows),
        _ => None,
    }
}

impl ApplicationRuntime {
    pub fn open(paths: RuntimePaths) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&paths.data_directory)?;
        let mut diagnostics = Vec::new();
        let history = match SqliteUsageRepository::open(&paths.data_directory.join("usage.sqlite3"))
        {
            Ok(history) => {
                Box::new(history) as Box<dyn crate::core::ranking::usage::UsageRepository>
            }
            Err(error) => {
                tracing::error!(error = %error, "usage_history_unavailable");
                diagnostics.push(format!(
                    "Usage history is unavailable; this session will not be saved: {error:#}"
                ));
                Box::new(crate::core::ranking::usage::MemoryUsage::default())
            }
        };
        let application = Arc::new(Application::new(
            anywhere_platform::native_platform()?,
            builtin_registry(),
            history,
            diagnostics,
        ));
        let settings = SettingsService::open(
            paths.data_directory.join("extensions.json"),
            application.as_ref(),
        )?;
        application.configure_web_storage(
            crate::persistence::module_storage::ModuleStorageHost::open(
                &paths.data_directory.join("web-storage.sqlite3"),
            )?,
        )?;

        let web_platform = resolve_web_platform(application.platform());
        let web_packages = match web_platform {
            Some(platform) => {
                let store = match WebPackageStore::open_with_builtins(
                    paths.data_directory.join("web-packages"),
                    platform,
                    crate::application::web_plugins::BUILTIN_WEB_PACKAGES,
                    application.as_ref(),
                ) {
                    Ok(store) => Some(store),
                    Err(error) => {
                        tracing::warn!(%error, "web_packages_unavailable");
                        application.publish_web_catalog(Default::default())?;
                        None
                    }
                };
                Mutex::new(store)
            }
            None => Mutex::new(None),
        };

        Ok(Arc::new(Self {
            application,
            settings: Arc::new(Mutex::new(settings)),
            web_packages: Arc::new(web_packages),
            web_platform,
            document: Mutex::new(None),
            opening: tokio::sync::Mutex::new(()),
            paths,
        }))
    }

    pub fn tool_window(
        &self,
        handle: &str,
    ) -> Result<(Arc<Self>, anywhere_protocol::LauncherState, String)> {
        let document = self.document(handle)?;
        let (application, state, invocation) = self.application.tool_window(&document)?;
        Ok((
            Arc::new(Self {
                application,
                settings: self.settings.clone(),
                paths: self.paths.clone(),
                web_packages: self.web_packages.clone(),
                web_platform: self.web_platform,
                document: Default::default(),
                opening: Default::default(),
            }),
            state,
            invocation,
        ))
    }

    pub fn history_reader(
        &self,
    ) -> Result<Arc<crate::features::application_history::service::HistoryService>> {
        self.settings()?.application_history()
    }

    pub fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    pub fn application(&self) -> &Arc<Application> {
        &self.application
    }

    pub fn settings(&self) -> RuntimeResult<MutexGuard<'_, SettingsService>> {
        self.settings.lock().map_err(|_| {
            RuntimeError::new(
                RuntimeErrorKind::Unavailable,
                "settings.unavailable",
                "Settings service unavailable",
            )
        })
    }

    pub fn web_packages(&self) -> RuntimeResult<MutexGuard<'_, Option<WebPackageStore>>> {
        self.web_packages.lock().map_err(|_| {
            RuntimeError::new(
                RuntimeErrorKind::Unavailable,
                "web.store_unavailable",
                "Web package store unavailable",
            )
        })
    }

    pub fn document(&self, handle: &str) -> RuntimeResult<WebDocumentBinding> {
        self.document
            .lock()
            .map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorKind::Unavailable,
                    "web.document_unavailable",
                    "Web document state unavailable",
                )
            })?
            .as_ref()
            .filter(|binding| binding.runtime_id() == handle)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::new(
                    RuntimeErrorKind::Stale,
                    "web.document_expired",
                    "Web document expired",
                )
            })
    }

    pub fn web_package_statuses(&self) -> Result<Vec<WebPackageStatus>> {
        self.web_packages()?
            .as_ref()
            .context("Web tools are unavailable")
            .map(WebPackageStore::statuses)
    }

    pub fn inspect_web_package(&self, path: &Path) -> Result<WebPackageReview> {
        self.web_packages()?
            .as_mut()
            .context("Web tools are unavailable")?
            .inspect(path)
    }

    pub fn cancel_web_review(&self, token: &str) -> Result<()> {
        self.web_packages()?
            .as_mut()
            .context("Web tools are unavailable")?
            .cancel_review(token);
        Ok(())
    }

    pub fn approve_web_package(&self, token: &str) -> Result<Vec<WebPackageStatus>> {
        self.web_packages()?
            .as_mut()
            .context("Web tools are unavailable")?
            .approve(self.application.as_ref(), token)?;
        self.web_package_statuses()
    }

    pub fn change_web_package(
        &self,
        id: &str,
        operation: WebPackageOperation,
    ) -> Result<Vec<WebPackageStatus>> {
        {
            let mut packages = self.web_packages()?;
            let store = packages.as_mut().context("Web tools are unavailable")?;
            match operation {
                WebPackageOperation::Enable => {
                    store.set_enabled(self.application.as_ref(), id, true)?;
                }
                WebPackageOperation::Disable => {
                    store.set_enabled(self.application.as_ref(), id, false)?;
                }
                WebPackageOperation::Uninstall => {
                    store.uninstall(self.application.as_ref(), id)?;
                }
                WebPackageOperation::Rollback => {
                    store.rollback(self.application.as_ref(), id)?;
                }
            }
        }
        self.web_package_statuses()
    }

    pub async fn open_web_document(
        &self,
        invocation_id: &str,
        resume_handle: Option<&str>,
    ) -> Result<WebDocumentBinding> {
        let platform = self
            .web_platform
            .context("Web tools are not enabled on this platform")?;
        let _opening = self.opening.lock().await;
        let binding = match resume_handle {
            Some(handle) => self.application.resume_web_document(
                &self.document(handle).map_err(anyhow::Error::new)?,
                platform,
            )?,
            None => self
                .application
                .bind_web_document(invocation_id, platform)?,
        };
        *self
            .document
            .lock()
            .map_err(|_| anyhow::anyhow!("Web document state unavailable"))? =
            Some(binding.clone());
        Ok(binding)
    }

    pub async fn open_web_document_view(
        &self,
        invocation_id: &str,
        resume_handle: Option<&str>,
    ) -> Result<WebDocumentView> {
        let binding = self.open_web_document(invocation_id, resume_handle).await?;
        let source = WebMessageSource {
            runtime_id: binding.runtime_id(),
            document_epoch: binding.document_epoch(),
            origin: binding.origin(),
            main_frame: true,
        };
        let invocation = self
            .application
            .web_invocation(&binding, source)
            .context("Web invocation unavailable")?;
        let package = self
            .web_package_statuses()?
            .into_iter()
            .find(|package| {
                package.id == invocation.plugin_id
                    && package.enabled
                    && package.digest == binding.content_digest()
            })
            .context("Web package unavailable")?;
        Ok(WebDocumentView {
            origin: binding.origin().into(),
            entry: package.entry,
            title: package.name,
            builtin: package.builtin,
            binding,
        })
    }

    pub fn close_web_document(&self, handle: &str) -> Result<()> {
        let binding = self.document(handle).map_err(anyhow::Error::new)?;
        self.application.close_web_document(&binding)?;
        let mut current = self
            .document
            .lock()
            .map_err(|_| anyhow::anyhow!("Web document state unavailable"))?;
        if current
            .as_ref()
            .is_some_and(|binding| binding.runtime_id() == handle)
        {
            *current = None;
        }
        Ok(())
    }

    pub async fn dispatch_web_frame(
        &self,
        handle: &str,
        origin: &str,
        main_frame: bool,
        frame: &str,
    ) -> Result<WebDispatchOutcome> {
        let binding = self.document(handle).map_err(anyhow::Error::new)?;
        Ok(self
            .application
            .dispatch_web_outcome(
                &binding,
                WebMessageSource {
                    runtime_id: binding.runtime_id(),
                    document_epoch: binding.document_epoch(),
                    origin,
                    main_frame,
                },
                frame,
            )
            .await)
    }

    /// Recheck the binding immediately before the frontend applies a window intent.
    pub fn validate_web_host_action(&self, action: &WebHostAction) -> Result<()> {
        let binding = self.document(&action.handle)?;
        anyhow::ensure!(
            binding.document_epoch() == action.document_epoch,
            "Web document changed"
        );
        self.application.validate_web_document(
            &binding,
            WebMessageSource {
                runtime_id: binding.runtime_id(),
                document_epoch: binding.document_epoch(),
                origin: binding.origin(),
                main_frame: true,
            },
        )
    }

    pub fn web_resource(&self, handle: &str, path: &str) -> Result<Vec<u8>> {
        let binding = self.document(handle).map_err(anyhow::Error::new)?;
        let source = || WebMessageSource {
            runtime_id: binding.runtime_id(),
            document_epoch: binding.document_epoch(),
            origin: binding.origin(),
            main_frame: true,
        };
        self.application.validate_web_document(&binding, source())?;
        let bytes = self
            .web_packages()?
            .as_ref()
            .context("Web tools are unavailable")?
            .resource(binding.content_digest(), path)?;
        self.application.validate_web_document(&binding, source())?;
        Ok(bytes)
    }

    pub async fn wait_web_state(&self, handle: &str, after: u64) -> Result<Option<String>> {
        let binding = self.document(handle).map_err(anyhow::Error::new)?;
        self.application
            .wait_web_state(&binding, after)
            .await
            .map_err(|_| anyhow::anyhow!("Plugin state unavailable"))
    }
}

pub fn canonical_existing(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    path.canonicalize()
        .with_context(|| format!("path does not exist: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_error_preserves_stable_code() {
        let error = RuntimeError::new(RuntimeErrorKind::Stale, "test.stale", "stale");
        assert_eq!(error.code, "test.stale");
        assert_eq!(error.kind, RuntimeErrorKind::Stale);
        let wrapped = anyhow::Error::new(error).context("frontend boundary");
        let restored = wrapped.downcast_ref::<RuntimeError>().unwrap();
        assert_eq!(restored.kind, RuntimeErrorKind::Stale);
        assert_eq!(restored.code, "test.stale");
    }

    #[test]
    fn runtime_paths_are_explicit() {
        let paths = RuntimePaths::new("/tmp/data", "/tmp/plugins");
        assert_eq!(paths.data_directory, PathBuf::from("/tmp/data"));
        assert_eq!(
            paths.bundled_plugin_directory,
            PathBuf::from("/tmp/plugins")
        );
    }
}
