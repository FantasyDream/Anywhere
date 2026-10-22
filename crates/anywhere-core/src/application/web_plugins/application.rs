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
        let session_id = {
            let mut session = self.session()?;
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("Web service unavailable"))?;
            let registry = web.prepare_registry(&catalog, &self.resolver.base_registry())?;
            web.replace_catalog(catalog);
            self.resolver.replace_base_registry(registry);
            self.resolver
                .action_catalog_version
                .fetch_add(1, Ordering::AcqRel);
            self.resolver.execution_epoch.fetch_add(1, Ordering::AcqRel);
            session.actions = self.resolver.base_registry();
            session.execution = Default::default();
            session.state.session_id
        };
        self.resolver.recompute_current(session_id);
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
