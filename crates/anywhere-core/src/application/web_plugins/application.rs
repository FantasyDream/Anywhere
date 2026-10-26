use super::{WebDocumentBinding, WebMessageSource, WebPluginCatalog};
use crate::application::Application;
use anyhow::{Context, Result};
use anywhere_protocol::web_plugin::{WebInvocation, WebPlatform};
use std::sync::atomic::Ordering;

impl Application {
    /// Resource reads validate identity without cloning the (up to 1 MiB) invocation.
    /// Call again after disk I/O so hiding/revocation cannot publish a stale response.
    pub fn validate_web_document(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<()> {
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
            .authorize(binding, source)?;
        Ok(())
    }
    pub fn web_document_is_background(&self, binding: &WebDocumentBinding) -> Result<bool> {
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?
            .is_background(
                binding,
                WebMessageSource {
                    runtime_id: binding.runtime_id(),
                    document_epoch: binding.document_epoch(),
                    origin: binding.origin(),
                    main_frame: true,
                },
            )
    }
    pub fn close_web_document(&self, binding: &WebDocumentBinding) -> Result<()> {
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
            .close_document(binding);
        Ok(())
    }
    /// Trusted Host composition boundary. No bridge exposes package approval/registration.
    /// Prepare every definition before swapping either catalog; errors leave both unchanged.
    pub fn publish_web_catalog(&self, catalog: WebPluginCatalog) -> Result<()> {
        let (session_id, restart) = {
            let mut session = self.session()?;
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("Web service unavailable"))?;
            let registry = web.prepare_registry(&catalog, &self.resolver.base_registry())?;
            let mut providers = self
                .resolver
                .providers
                .lock()
                .map_err(|_| anyhow::anyhow!("providers unavailable"))?;
            let next_providers = catalog.append_providers(
                &providers,
                self.resolver.session.clone(),
                self.platform.clone(),
            )?;
            let restart = [&web.catalog, &catalog].iter().any(|catalog| {
                catalog.entries.values().any(|entry| {
                    entry
                        .manifest
                        .background
                        .as_ref()
                        .is_some_and(|b| !b.resolvers.is_empty())
                })
            });
            let revision = if restart && !self.resolver.is_abandoned() {
                Some(
                    session
                        .state
                        .revision
                        .checked_add(1)
                        .context("query revision exhausted")?,
                )
            } else {
                None
            };
            web.replace_catalog(catalog);
            *providers = next_providers;
            self.resolver.replace_base_registry(registry);
            self.resolver
                .action_catalog_version
                .fetch_add(1, Ordering::AcqRel);
            self.resolver.execution_epoch.fetch_add(1, Ordering::AcqRel);
            session.actions = self.resolver.base_registry();
            session.execution = Default::default();
            if restart {
                session
                    .resolution
                    .committed_contexts
                    .retain(|context| !context.type_id.starts_with("plugin."));
                session
                    .resolution
                    .context_cache
                    .retain(|context| !context.type_id.starts_with("plugin."));
            }
            let restart = if let Some(revision) = revision {
                let input = session.state.input.clone();
                session.begin_revision(revision, input.clone());
                self.resolver
                    .mark_current(session.state.session_id, revision);
                Some((
                    revision,
                    crate::application::session_runtime::seeds_from(&session, &input),
                ))
            } else {
                None
            };
            (session.state.session_id, restart)
        };
        if let Some((revision, seeds)) = restart {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let resolver = self.resolver.clone();
                runtime.spawn(async move {
                    let started = std::time::Instant::now();
                    let run = resolver.build_run(&resolver.execution(session_id, revision), seeds);
                    if let Some(resolution) = resolver.resolve_until(session_id, run, started).await
                    {
                        let _ = resolver.commit(
                            session_id,
                            revision,
                            resolution.contexts,
                            resolution.diagnostics,
                            resolution.complete,
                            started,
                        );
                    }
                });
            } else {
                // Synchronous embedders without a scheduler must reactivate rather
                // than reuse a graph whose third-party authority has changed.
                self.resolver.abandon_session();
            }
        } else {
            self.resolver.recompute_current(session_id);
        }
        self.refresh_app_actions(session_id, None);
        Ok(())
    }

    pub fn bind_web_document(
        &self,
        invocation_id: &str,
        platform: WebPlatform,
    ) -> Result<WebDocumentBinding> {
        self.attach_web_document(invocation_id, platform, None)
    }
    pub fn resume_web_document(
        &self,
        old: &WebDocumentBinding,
        platform: WebPlatform,
    ) -> Result<WebDocumentBinding> {
        self.attach_web_document(&old.invocation_id, platform, Some(old))
    }
    fn attach_web_document(
        &self,
        invocation_id: &str,
        platform: WebPlatform,
        old: Option<&WebDocumentBinding>,
    ) -> Result<WebDocumentBinding> {
        let runtime =
            tokio::runtime::Handle::try_current().context("Web Host runtime unavailable")?;
        let session = self.session()?;
        anyhow::ensure!(
            !session.suspended && !self.resolver.is_abandoned(),
            "Launcher is not interactive"
        );
        let binding = {
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("Web service unavailable"))?;
            match old {
                Some(old) => web.resume(old, platform)?,
                None => web.bind(invocation_id, platform)?,
            }
        };
        let weak = std::sync::Arc::downgrade(&self.web_plugins);
        let document = binding.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let Some(web) = weak.upgrade() else { break };
                let active = web
                    .lock()
                    .is_ok_and(|mut web| web.sweep_document(&document));
                if !active {
                    break;
                }
            }
        });
        Ok(binding)
    }

    pub fn web_invocation(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<WebInvocation> {
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
            .invocation(binding, source)
    }
}
