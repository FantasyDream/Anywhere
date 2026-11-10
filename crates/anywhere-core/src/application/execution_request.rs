//! A single committed execution request, frozen before its worker is queued.
//!
//! The request owns the candidate, source identity and authorization versions. Shared
//! handles retain the same application services; they never create another session.
use super::{session_runtime::Resolver, Application};
use crate::{
    core::{
        action::{
            descriptor::ActionRegistry, handler::ExecutionOutput, identity::BusinessActionId,
        },
        context::ExecutionContext,
        execution::ExecutionSessionManager,
        search::candidate::Candidate,
    },
    features::{filesystem::search::FileSearchPolicy, policy::FeaturePolicySnapshot},
};
use anyhow::{bail, Context, Result};
use anywhere_platform::{ApplicationInfo, Platform};
use anywhere_protocol::*;
use std::sync::{atomic::Ordering, Arc, Mutex, RwLock};

mod worker;

/// Consumed by spawn; hide/resume cannot refresh an already prepared request.
pub(super) struct ExecutionRequest {
    resolver: Resolver,
    candidate: Candidate,
    source_app: String,
    captured_app: Option<ApplicationInfo>,
    pub menu_generation: u64,
    pub source_bundle: Option<String>,
    catalog_version: u64,
    feature_policy: Arc<FeaturePolicySnapshot>,
    execution_epoch: u64,
    file_policy: Arc<RwLock<FileSearchPolicy>>,
    file_generation: u64,
    platform: Arc<dyn Platform>,
    registry: Arc<ActionRegistry>,
    executions: Arc<ExecutionSessionManager>,
    web_plugins: Arc<Mutex<super::web_plugins::WebPluginService>>,
    execution_ticket: ExecutionContext,
    record_id: BusinessActionId,
    context_type: String,
}

pub(super) struct ExecutionOutcome {
    pub output: ExecutionOutput,
    pub history_result: Result<()>,
    pub execution_session_id: Option<ExecutionSessionId>,
}

impl ExecutionRequest {
    pub fn prepare(
        app: &Application,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<Self> {
        let execution_epoch = app.resolver.execution_epoch.load(Ordering::Acquire);
        let file_policy = app.file_search_policy.clone();
        let file_generation = file_policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let (
            candidate,
            source_app,
            captured_app,
            menu_generation,
            source_bundle,
            catalog_version,
            feature_policy,
        ) = {
            let session = app.session()?;
            if session.suspended
                || session.state.session_id != session_id
                || app
                    .resolver
                    .current
                    .load(std::sync::atomic::Ordering::Acquire)
                    != crate::core::context::engine::stamp(session_id, session.state.revision)
                || session.state.revision != revision
            {
                bail!("stale session");
            }
            // The revision is current, but its candidates may not be published yet:
            // `update_input` makes the new revision current before it resolves, so the
            // map still holds the previous revision's candidates. They carry the previous
            // context and execution target, so serving them here would execute an action
            // that belongs to an input the user has already replaced.
            let catalog_version = app.resolver.catalog.version();
            if session.execution.revision != Some(revision)
                || session.execution.action_catalog_version != Some(catalog_version)
            {
                bail!("action expired; choose a current action");
            }
            let candidate = session
                .execution
                .candidates
                .get(candidate_id.as_str())
                .cloned()
                .context("action expired; choose a current action")?;
            let feature_policy = app.resolver.catalog.policy();
            if let crate::core::search::candidate::CandidateTarget::Action { action, .. } =
                &candidate.target
            {
                if action
                    .contributor()
                    .is_some_and(|id| !app.resolver.catalog.allows(id))
                {
                    bail!("feature is disabled; choose a current action");
                }
            }
            if !candidate.presentation.availability.is_executable() {
                bail!(
                    "{}",
                    candidate
                        .presentation
                        .availability
                        .reason
                        .clone()
                        .unwrap_or_else(|| "action is not available".into())
                );
            }
            (
                candidate,
                session.identity.source_app.clone(),
                session.identity.frontmost.clone(),
                session.menu.generation,
                session
                    .identity
                    .frontmost
                    .as_ref()
                    .map(|app| app.bundle_id.clone()),
                catalog_version,
                feature_policy,
            )
        };
        let platform = app.platform.clone();
        let registry = app.resolver.base_registry();
        let executions = app.executions.clone();
        let web_plugins = app.web_plugins.clone();
        // Handed to the worker so it can re-check currency when it actually starts running,
        // rather than trusting a check made while the request was still being queued.
        let execution_ticket = app.resolver.execution(session_id, revision);
        let record_id = crate::core::action::identity::BusinessActionId::new(
            candidate
                .presentation
                .command_id
                .clone()
                .or_else(|| {
                    candidate
                        .presentation
                        .bundle_id
                        .as_ref()
                        .map(|bundle| crate::core::ranking::usage::application_usage_id(bundle))
                })
                .context("candidate has no executable identity")?,
        );
        let context_type = candidate
            .context()
            .map(|context| context.type_id.clone())
            .unwrap_or_else(|| crate::core::ranking::usage::APPLICATION_CONTEXT_TYPE.to_owned());
        tracing::info!(
            session_id,
            revision,
            candidate_id,
            command_id = record_id.as_str(),
            "action_execution_started"
        );
        Ok(Self {
            resolver: app.resolver.clone(),
            candidate,
            source_app,
            captured_app,
            menu_generation,
            source_bundle,
            catalog_version,
            feature_policy,
            execution_epoch,
            file_policy,
            file_generation,
            platform,
            registry,
            executions,
            web_plugins,
            execution_ticket,
            record_id,
            context_type,
        })
    }

    fn is_current(&self) -> bool {
        self.execution_ticket.is_current()
            && self
                .resolver
                .catalog
                .is_policy_current(&self.feature_policy)
            && self.resolver.execution_epoch.load(Ordering::Acquire) == self.execution_epoch
            && self.resolver.catalog.version() == self.catalog_version
    }
}
