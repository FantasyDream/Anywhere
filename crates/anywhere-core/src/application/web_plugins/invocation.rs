use super::{
    catalog::WebPluginCatalog, diagnostics::Diagnostics, interaction::InteractionPort, projection,
};
use crate::core::{
    action::{descriptor::ActionRegistry, web_command::WebCommandRef},
    context::ResolvedContext,
};
use anyhow::{ensure, Context, Result};
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::{
    web_plugin::{WebCurrentApp, WebInvocation, WebPermission, WebPlatform},
    HostView, Revision, SessionId,
};
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;

/// Issued only to trusted Host code. It is not parsed from browser request bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebDocumentBinding {
    pub(super) invocation_id: String,
    runtime_id: String,
    document_epoch: u64,
    origin: String,
    content_digest: String,
}
impl WebDocumentBinding {
    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }
    pub fn document_epoch(&self) -> u64 {
        self.document_epoch
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }
}

/// Values supplied by the native message callback, never copied from JS payload fields.
#[derive(Clone, Copy)]
pub struct WebMessageSource<'a> {
    pub runtime_id: &'a str,
    pub document_epoch: u64,
    pub origin: &'a str,
    pub main_frame: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct UsageRecord {
    pub action_id: String,
    pub context_type: String,
    pub source_app: String,
}

pub(super) struct Invocation {
    pub(super) command: WebCommandRef,
    pub(super) input: WebInvocation,
    pub(super) session_id: anywhere_protocol::SessionId,
    pub(super) launch_revision: Revision,
    pub(super) resource: projection::Resources,
    app: Option<WebCurrentApp>,
    binding: Option<WebDocumentBinding>,
    hidden_at: Option<Instant>,
    pub(super) document_epoch: u64,
    usage_recorded: bool,
    pub(super) lifecycle: RuntimeLifecycle,
    pub(super) usage: UsageRecord,
    pub(super) rpc: super::rpc_state::RpcState,
    pub(super) closed: tokio::sync::watch::Sender<bool>,
}

pub(super) enum RuntimeLifecycle {
    Webview { ready: bool, bound_at: Instant },
    Background { state: BackgroundState },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BackgroundState {
    Running,
    AwaitingInteraction,
    HandoffCompleted { request_id: Option<String> },
}

impl Invocation {
    pub(super) fn is_draining(&self) -> bool {
        self.hidden_at.is_some() && self.handed_off()
    }

    pub(super) fn handed_off(&self) -> bool {
        matches!(
            self.lifecycle,
            RuntimeLifecycle::Background {
                state: BackgroundState::HandoffCompleted { .. }
            }
        )
    }

    pub(super) fn handoff_request_id(&self) -> Option<&str> {
        match &self.lifecycle {
            RuntimeLifecycle::Background {
                state: BackgroundState::HandoffCompleted { request_id },
            } => request_id.as_deref(),
            _ => None,
        }
    }
}
impl RuntimeLifecycle {
    fn webview() -> Self {
        Self::Webview {
            ready: false,
            bound_at: Instant::now(),
        }
    }

    fn waiting_for_ready_timed_out(&self) -> bool {
        matches!(self, Self::Webview {ready: false, bound_at} if bound_at.elapsed() >= Duration::from_secs(10))
    }

    pub(super) fn role(&self) -> anywhere_protocol::web_bridge::WebRole {
        match self {
            Self::Webview { .. } => anywhere_protocol::web_bridge::WebRole::Webview,
            Self::Background { .. } => anywhere_protocol::web_bridge::WebRole::Background,
        }
    }
}

#[derive(Default)]
pub struct WebPluginService {
    pub(super) interactions: InteractionPort,
    pub(super) catalog: WebPluginCatalog,
    pub(super) invocations: BTreeMap<String, Invocation>,
    pub(super) backgrounds:
        BTreeMap<String, std::sync::Arc<super::background_action::BackgroundJob>>,
    pub(super) diagnostics: Diagnostics,
    pub(super) storage: Option<crate::persistence::module_storage::ModuleStorageHost>,
}

impl WebPluginService {
    /// An explicitly opened window gets its own document, origin, RPC state and
    /// cancellation. Only the previously approved invocation input is copied,
    /// and standalone windows are available for all active Web plugins (built-in
    /// and third-party).
    pub(crate) fn tool_window(
        &mut self,
        document: &WebDocumentBinding,
        session_id: SessionId,
        revision: Revision,
    ) -> Result<(Self, String)> {
        let mut window = Self {
            catalog: self.catalog.clone(),
            storage: self.storage.clone(),
            diagnostics: self.diagnostics.clone(),
            ..Default::default()
        };
        let parent = self.authorize(
            document,
            WebMessageSource {
                runtime_id: document.runtime_id(),
                document_epoch: document.document_epoch(),
                origin: document.origin(),
                main_frame: true,
            },
        )?;
        ensure!(parent.session_id == session_id, "Web invocation expired");
        ensure!(
            parent.lifecycle.role() == anywhere_protocol::web_bridge::WebRole::Webview,
            "standalone windows require a WebView"
        );
        let mut input = parent.input.clone();
        let mut resource = parent.resource.clone();
        if let anywhere_protocol::web_plugin::WebInput::Resources { items, .. } = &mut input.input {
            for item in items {
                let handle = super::token()?;
                if let Some(resource) = resource
                    .as_mut()
                    .filter(|resource| resource.handle == item.handle)
                {
                    resource.handle.clone_from(&handle);
                    resource.access = Default::default();
                }
                item.handle = handle;
            }
        }
        let copied = Invocation {
            command: parent.command.clone(),
            input,
            session_id,
            launch_revision: revision,
            resource,
            app: parent.app.clone(),
            binding: None,
            hidden_at: None,
            document_epoch: 0,
            usage_recorded: true,
            lifecycle: RuntimeLifecycle::webview(),
            usage: parent.usage.clone(),
            rpc: Default::default(),
            closed: tokio::sync::watch::channel(false).0,
        };
        ensure!(
            self.catalog.entries.contains_key(&copied.command.plugin_id),
            "Web package unavailable"
        );
        let id = super::token()?;
        window.invocations.insert(id.clone(), copied);
        Ok((window, id))
    }

    /// Native view disposal may arrive after a reload/resume has rebound the same
    /// invocation. Only its current document generation can end the retained page.
    pub fn close_document(&mut self, binding: &WebDocumentBinding) {
        if self
            .invocations
            .get(&binding.invocation_id)
            .is_some_and(|invocation| invocation.document_epoch == binding.document_epoch)
        {
            self.invocations.remove(&binding.invocation_id);
            if let Some(job) = self.backgrounds.remove(binding.runtime_id()) {
                job.cancel();
            }
        }
    }
    pub(crate) fn restore_providers(
        &self,
        providers: &crate::core::context::registry::ContextProviderRegistry,
        session: std::sync::Arc<crate::application::session_coordinator::SessionCoordinator>,
        platform: std::sync::Arc<dyn anywhere_platform::Platform>,
    ) -> Result<crate::core::context::registry::ContextProviderRegistry> {
        self.catalog
            .append_providers(providers, session, platform, &self.diagnostics)
    }
    pub(crate) fn restore_actions(&self, base: &ActionRegistry) -> Result<ActionRegistry> {
        self.catalog.append_to(base)
    }
    pub(super) fn prepare_registry(
        &self,
        next: &WebPluginCatalog,
        base: &ActionRegistry,
    ) -> Result<ActionRegistry> {
        for (id, entry) in &next.entries {
            if let Some(old) = self.catalog.entries.get(id) {
                ensure!(
                    entry.source_id == old.source_id,
                    "Web package source changed; explicit replacement is required"
                );
            }
        }
        next.append_to(base)
    }
    pub(super) fn replace_catalog(&mut self, catalog: WebPluginCatalog) {
        self.invalidate();
        for entry in self.catalog.entries.values() {
            entry
                .active
                .store(false, std::sync::atomic::Ordering::Release);
        }
        self.catalog = catalog;
    }
    pub(crate) fn invalidate(&mut self) {
        self.invocations.clear();
        for job in self.backgrounds.values() {
            job.cancel();
        }
        self.backgrounds.clear();
    }

    pub(crate) fn start(
        &mut self,
        command: &WebCommandRef,
        context: &ResolvedContext,
        session_id: SessionId,
        launch_revision: Revision,
        app: Option<&ApplicationInfo>,
        source_app: &str,
    ) -> Result<HostView> {
        let entry = self
            .catalog
            .entries
            .get(&command.plugin_id)
            .context("Web package is not active")?;
        ensure!(
            entry.token == command.registration,
            "Web command registration expired"
        );
        let definition = entry
            .manifest
            .commands
            .iter()
            .find(|c| c.id == command.command_id)
            .context("Web command no longer exists")?;
        let workspace = context.type_id == crate::features::tools::ENTRY_CONTEXT;
        ensure!(
            (workspace && !definition.needs_context())
                || definition.accepts.contains(&context.type_id),
            "Web command does not accept this object"
        );
        if definition.require_input_match
            || (context.source == anywhere_protocol::ContextSource::UserInput
                && context.type_id == anywhere_protocol::TEXT)
        {
            ensure!(
                context.type_id == anywhere_protocol::TEXT,
                "Web command requires recognized text input"
            );
            let text = match &context.value {
                crate::core::context::ContextValue::Text(text) => text.as_ref(),
                _ => anyhow::bail!("invalid Web text input"),
            };
            let mut matches = crate::core::context::input_patterns::InputPatternMatches::new(text);
            ensure!(
                matches.any(&definition.input_matchers),
                "Web command does not recognize this input"
            );
        }
        let (mut input, resources) =
            projection::build_with_resources(context, &entry.manifest.permissions)?;
        if !definition.runtime.is_webview() {
            if let anywhere_protocol::web_plugin::WebInput::Resources { items, .. } = &mut input {
                for item in items {
                    item.previewable = false;
                }
            }
        }
        let id = super::token()?;
        let invocation = Invocation {
            command: command.clone(),
            session_id,
            // The accepted Context can be a revision-0 launcher entry. Only the
            // committed execution request identifies the input revision at launch.
            launch_revision,
            input: WebInvocation {
                capabilities: anywhere_protocol::web_plugin::WebCapabilities {
                    url_open: entry
                        .manifest
                        .permissions
                        .iter()
                        .any(|permission| matches!(permission, WebPermission::UrlOpen { .. })),
                    window_control: definition.runtime.is_webview(),
                    resource_read: resources.is_some()
                        && entry
                            .manifest
                            .permissions
                            .contains(&WebPermission::ResourcesRead {}),
                    preview_open: definition.runtime.is_webview()
                        && resources.is_some()
                        && entry
                            .manifest
                            .permissions
                            .contains(&WebPermission::PreviewOpen {}),
                },
                plugin_id: command.plugin_id.clone(),
                command_id: command.command_id.clone(),
                entrypoint: definition.entrypoint.clone(),
                input,
            },
            resource: resources,
            app: app.map(|app| WebCurrentApp {
                id: app.bundle_id.clone(),
                name: app.name.clone(),
            }),
            binding: None,
            hidden_at: None,
            document_epoch: 0,
            usage_recorded: false,
            lifecycle: match definition.runtime {
                anywhere_protocol::web_plugin::PluginRuntime::Webview => {
                    RuntimeLifecycle::webview()
                }
                anywhere_protocol::web_plugin::PluginRuntime::Background => {
                    RuntimeLifecycle::Background {
                        state: BackgroundState::Running,
                    }
                }
            },
            rpc: Default::default(),
            closed: tokio::sync::watch::channel(false).0,
            usage: UsageRecord {
                action_id: command.action_id(),
                context_type: context.type_id.clone(),
                source_app: source_app.into(),
            },
        };
        // One active tool by default. Replacement cancels its finite background job too.
        self.invalidate();
        self.invocations.insert(id.clone(), invocation);
        Ok(HostView::WebPlugin { invocation_id: id })
    }

    /// Attach/reload/resume is a trusted Host operation, not an SDK method.
    /// Every attach has a fresh runtime and epoch; an old document cannot reuse its binding.
    pub fn bind(
        &mut self,
        invocation_id: &str,
        platform: WebPlatform,
    ) -> Result<WebDocumentBinding> {
        self.prune();
        let invocation = self
            .invocations
            .get_mut(invocation_id)
            .context("Web invocation expired")?;
        let entry = self
            .catalog
            .entries
            .get(&invocation.command.plugin_id)
            .context("Web package revoked")?;
        ensure!(
            entry.active.load(std::sync::atomic::Ordering::Acquire)
                && entry.token == invocation.command.registration
                && entry.manifest.platforms.contains(&platform),
            "Web invocation no longer qualified"
        );
        let runtime_id = super::token()?;
        let document_epoch = invocation
            .document_epoch
            .checked_add(1)
            .context("Web document epoch exhausted")?;
        let binding = WebDocumentBinding {
            invocation_id: invocation_id.into(),
            origin: format!("anywhere-plugin://{runtime_id}.plugin.local"),
            runtime_id,
            document_epoch,
            content_digest: entry.digest.clone(),
        };
        invocation.document_epoch = document_epoch;
        invocation.rpc = Default::default();
        invocation.closed = tokio::sync::watch::channel(false).0;
        if matches!(invocation.lifecycle, RuntimeLifecycle::Webview { .. }) {
            invocation.lifecycle = RuntimeLifecycle::webview();
        }
        invocation.binding = Some(binding.clone());
        invocation.hidden_at = None;
        Ok(binding)
    }

    /// A retained native view keeps its origin but receives a new epoch/runtime.
    /// Only the opaque prior Host binding can request this, never a browser field.
    pub fn resume(
        &mut self,
        old: &WebDocumentBinding,
        platform: WebPlatform,
    ) -> Result<WebDocumentBinding> {
        self.prune();
        let invocation = self
            .invocations
            .get(&old.invocation_id)
            .context("Web invocation expired")?;
        ensure!(
            invocation.hidden_at.is_some() && invocation.document_epoch == old.document_epoch,
            "Web document is not retained"
        );
        ensure!(
            self.catalog
                .entries
                .get(&invocation.command.plugin_id)
                .is_some_and(|r| r.digest == old.content_digest),
            "Web package changed"
        );
        let mut next = self.bind(&old.invocation_id, platform)?;
        next.origin = old.origin.clone();
        self.invocations
            .get_mut(&old.invocation_id)
            .context("Web invocation expired")?
            .binding = Some(next.clone());
        Ok(next)
    }

    pub fn invocation(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<WebInvocation> {
        Ok(self.authorize(binding, source)?.input.clone())
    }

    pub fn is_background(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            self.authorize(binding, source)?.lifecycle,
            RuntimeLifecycle::Background { .. }
        ))
    }

    pub fn current_app(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<Option<WebCurrentApp>> {
        let (plugin_id, app) = {
            let invocation = self.authorize(binding, source)?;
            (invocation.command.plugin_id.clone(), invocation.app.clone())
        };
        let entry = &self.catalog.entries[&plugin_id];
        ensure!(
            entry
                .manifest
                .permissions
                .iter()
                .any(|p| matches!(p, WebPermission::CurrentApp {})),
            "current application permission required"
        );
        Ok(app)
    }

    pub(super) fn ready(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<Option<UsageRecord>> {
        self.authorize(binding, source)?;
        let invocation = self
            .invocations
            .get_mut(&binding.invocation_id)
            .context("Web invocation expired")?;
        let RuntimeLifecycle::Webview { ready, .. } = &mut invocation.lifecycle else {
            anyhow::bail!("ready requires a WebView");
        };
        *ready = true;
        if std::mem::replace(&mut invocation.usage_recorded, true) {
            return Ok(None);
        }
        Ok(Some(invocation.usage.clone()))
    }

    pub(crate) fn suspend(&mut self) {
        for invocation in self.invocations.values_mut() {
            if invocation.handed_off() {
                let keep = invocation.handoff_request_id().map(str::to_owned);
                invocation.rpc.cancel_except(keep.as_deref());
                invocation.closed.send_replace(true);
                invocation.hidden_at.get_or_insert_with(Instant::now);
                continue;
            }
            if let Some(binding) = invocation.binding.take() {
                if let Some(job) = self.backgrounds.remove(binding.runtime_id()) {
                    job.cancel();
                }
            }
            invocation.closed.send_replace(true);
            invocation.rpc = Default::default();
            // Repeated focus notifications must not restart the retention interval.
            invocation.hidden_at.get_or_insert_with(Instant::now);
        }
    }

    fn prune(&mut self) {
        self.invocations.retain(|_, invocation| {
            if invocation.hidden_at.is_none()
                && invocation.binding.is_some()
                && invocation.lifecycle.waiting_for_ready_timed_out()
            {
                return false;
            }
            invocation.hidden_at.is_none_or(|at| {
                at.elapsed()
                    < Duration::from_millis(anywhere_protocol::policy::LAUNCHER_RETENTION_MS)
            })
        });
    }

    pub(super) fn authorize(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> Result<&Invocation> {
        self.prune();
        let invocation = self
            .invocations
            .get(&binding.invocation_id)
            .context("Web invocation expired")?;
        let current = invocation
            .binding
            .as_ref()
            .context("Web invocation is not interactive")?;
        let entry = self
            .catalog
            .entries
            .get(&invocation.command.plugin_id)
            .context("Web package revoked")?;
        ensure!(
            entry.active.load(std::sync::atomic::Ordering::Acquire)
                && entry.token == invocation.command.registration,
            "Web grant expired"
        );
        ensure!(
            current.runtime_id == binding.runtime_id
                && current.document_epoch == binding.document_epoch
                && current.content_digest == binding.content_digest
                && current.content_digest == entry.digest
                && source.main_frame
                && source.runtime_id == current.runtime_id
                && source.document_epoch == current.document_epoch
                && source.origin == current.origin,
            "Web document source mismatch"
        );
        Ok(invocation)
    }
}
