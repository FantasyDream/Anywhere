//! A normal tool window has a separate session while the launcher remains available.
use super::{
    session_model::LauncherSession, web_plugins::WebDocumentBinding, Application,
    LauncherApplication,
};
use anyhow::{ensure, Result};
use std::sync::{Arc, Mutex};

impl Application {
    pub(super) fn tool_window(
        &self,
        document: &WebDocumentBinding,
    ) -> Result<(Arc<Self>, anywhere_protocol::LauncherState, String)> {
        let parent = self.session()?;
        let id = parent.state.session_id;
        ensure!(
            id != 0 && !parent.suspended && self.resolver.is_session_active(id),
            "launcher session expired"
        );
        let revision = parent.state.revision;
        let (web, invocation) = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
            .tool_window(document, id, revision)?;
        let resolver = self.resolver.tool_window();
        {
            let mut session = resolver.session.access()?;
            *session = LauncherSession::reserved(id, resolver.base_registry());
            session.state.input = parent.state.input.clone();
            session.state.revision = revision;
            session.identity.source_app = parent.identity.source_app.clone();
            session.identity.frontmost = parent.identity.frontmost.clone();
            session.usage = parent.usage.clone();
            session.app_usage = parent.app_usage.clone();
            session.resolution.committed_revision = Some(revision);
            session.execution.revision = Some(revision);
            session.execution.action_catalog_version = Some(resolver.catalog.version());
            resolver.mark_current(id, revision);
        }
        let child = Arc::new(Self {
            tool_windows: Default::default(),
            object_selection_order: Default::default(),
            previews: self.previews.clone(),
            web_rpc_workers: self.web_rpc_workers.clone(),
            web_plugins: Arc::new(Mutex::new(web)),
            executions: self.executions.clone(),
            platform: self.platform.clone(),
            startup_diagnostics: Vec::new(),
            budget: self.budget,
            content_grace: self.content_grace,
            app_actions: self.app_actions.clone(),
            file_search_policy: self.file_search_policy.clone(),
            file_search_source: Arc::new(self.file_search_source.tool_window()),
            isolated_file_search: true,
            preview_worker: self.preview_worker.clone(),
            context_capture: super::context_capture::ContextCaptureService::new(
                self.platform.clone(),
                resolver.clone(),
            ),
            resolver,
        });
        let mut windows = crate::core::sync::lock_for_revoke(&self.tool_windows, "tool_windows");
        windows.retain(|window| window.strong_count() != 0);
        windows.push(Arc::downgrade(&child));
        let state = child.session()?.state.clone();
        Ok((child, state, invocation))
    }

    // Called under the parent's publication lock. Existing windows keep their UI
    // while work authorized by the previous configuration is revoked immediately.
    pub(super) fn revoke_tool_windows(&self) {
        let windows = std::mem::take(&mut *crate::core::sync::lock_for_revoke(
            &self.tool_windows,
            "tool_windows",
        ));
        for child in windows.into_iter().filter_map(|window| window.upgrade()) {
            let id = child.resolver.session.access_for_revoke().state.session_id;
            let _ = child.deactivate(id);
        }
    }
}
