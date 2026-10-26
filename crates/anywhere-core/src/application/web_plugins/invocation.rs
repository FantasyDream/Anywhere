use super::{catalog::WebPluginCatalog, projection};
use crate::core::{
    action::{descriptor::ActionRegistry, web_command::WebCommandRef},
    context::ResolvedContext,
};
use anyhow::{ensure, Context, Result};
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::{
    web_plugin::{WebCurrentApp, WebInvocation, WebPermission, WebPlatform},
    HostView,
};
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;

/// Issued only to trusted Host code. It is not parsed from browser request bodies.
#[derive(Clone)]
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
pub struct WebMessageSource<'a> {
    pub runtime_id: &'a str,
    pub document_epoch: u64,
    pub origin: &'a str,
    pub main_frame: bool,
}

pub(super) struct UsageRecord {
    pub action_id: String,
    pub context_type: String,
    pub source_app: String,
}

pub(super) struct Invocation {
    pub(super) command: WebCommandRef,
    input: WebInvocation,
    // Paths stay private; the public broker grants no filesystem operation.
    _resources: projection::Resources,
    app: Option<WebCurrentApp>,
    binding: Option<WebDocumentBinding>,
    hidden_at: Option<Instant>,
    document_epoch: u64,
    usage_recorded: bool,
    document_ready: bool,
    usage: UsageRecord,
    pub(super) rpc: super::rpc_state::RpcState,
    bound_at: Instant,
}

#[derive(Default)]
pub struct WebPluginService {
    pub(super) catalog: WebPluginCatalog,
    pub(super) invocations: BTreeMap<String, Invocation>,
    pub(super) storage: Option<crate::persistence::module_storage::ModuleStorageHost>,
}

impl WebPluginService {
    /// Native view disposal may arrive after a reload/resume has rebound the same
    /// invocation. Only its current document generation can end the retained page.
    pub fn close_document(&mut self, binding: &WebDocumentBinding) {
        if self
            .invocations
            .get(&binding.invocation_id)
            .is_some_and(|invocation| invocation.document_epoch == binding.document_epoch)
        {
            self.invocations.remove(&binding.invocation_id);
        }
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
        self.catalog = catalog;
    }
    pub(crate) fn invalidate(&mut self) {
        self.invocations.clear();
    }

    pub(crate) fn start(
        &mut self,
        command: &WebCommandRef,
        context: &ResolvedContext,
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
            workspace || definition.accepts.contains(&context.type_id),
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
        let (input, resources) =
            projection::build_with_resources(context, &entry.manifest.permissions)?;
        let id = super::token()?;
        let invocation = Invocation {
            command: command.clone(),
            input: WebInvocation {
                plugin_id: command.plugin_id.clone(),
                command_id: command.command_id.clone(),
                entrypoint: definition.entrypoint.clone(),
                input,
            },
            _resources: resources,
            app: app.map(|app| WebCurrentApp {
                id: app.bundle_id.clone(),
                name: app.name.clone(),
            }),
            binding: None,
            hidden_at: None,
            document_epoch: 0,
            usage_recorded: false,
            document_ready: false,
            rpc: Default::default(),
            bound_at: Instant::now(),
            usage: UsageRecord {
                action_id: command.action_id(),
                context_type: context.type_id.clone(),
                source_app: source_app.into(),
            },
        };
        // One live page by default. Replacement drops prior data and tickets atomically.
        self.invocations.clear();
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
            entry.token == invocation.command.registration
                && entry.manifest.platforms.contains(&platform),
            "Web invocation no longer qualified"
        );
        let runtime_id = super::token()?;
        let document_epoch = invocation
            .document_epoch
            .checked_add(1)
            .context("Web document epoch exhausted")?;
        let scheme = match platform {
            WebPlatform::Macos => "anywhere-plugin",
            WebPlatform::Windows => "https",
        };
        let binding = WebDocumentBinding {
            invocation_id: invocation_id.into(),
            origin: format!("{scheme}://{runtime_id}.plugin.local"),
            runtime_id,
            document_epoch,
            content_digest: entry.digest.clone(),
        };
        invocation.document_epoch = document_epoch;
        invocation.rpc = Default::default();
        invocation.bound_at = Instant::now();
        invocation.document_ready = false;
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
        if !invocation.document_ready && invocation.bound_at.elapsed() >= Duration::from_secs(10) {
            anyhow::bail!("Web ready deadline expired");
        }
        invocation.document_ready = true;
        if std::mem::replace(&mut invocation.usage_recorded, true) {
            return Ok(None);
        }
        Ok(Some(UsageRecord {
            action_id: invocation.usage.action_id.clone(),
            context_type: invocation.usage.context_type.clone(),
            source_app: invocation.usage.source_app.clone(),
        }))
    }

    pub(crate) fn suspend(&mut self) {
        for invocation in self.invocations.values_mut() {
            invocation.binding = None;
            invocation.rpc = Default::default();
            // Repeated focus notifications must not restart the retention interval.
            invocation.hidden_at.get_or_insert_with(Instant::now);
        }
    }

    fn prune(&mut self) {
        self.invocations.retain(|_, invocation| {
            if invocation.hidden_at.is_none()
                && invocation.binding.is_some()
                && !invocation.document_ready
                && invocation.bound_at.elapsed() >= Duration::from_secs(10)
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
            entry.token == invocation.command.registration,
            "Web grant expired"
        );
        ensure!(
            current.runtime_id == binding.runtime_id
                && current.document_epoch == binding.document_epoch
                && source.main_frame
                && source.runtime_id == current.runtime_id
                && source.document_epoch == current.document_epoch
                && source.origin == current.origin,
            "Web document source mismatch"
        );
        Ok(invocation)
    }
}
