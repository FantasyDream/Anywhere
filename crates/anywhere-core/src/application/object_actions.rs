//! Resolve operations only for the object the user opened, not every loaded row.
use super::Application;
use crate::{
    core::{ranking::score::UsageStats, search::engine::ActionSearch},
    features::filesystem::search,
};
use anyhow::{bail, Context, Result};
use anywhere_protocol::*;
use std::sync::atomic::Ordering;
impl Application {
    pub fn file_actions(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<Vec<CandidatePresentation>> {
        let (file, registry, facts, usage, epoch, catalog, inputs) = {
            let session = self.session()?;
            if session.suspended
                || !self.resolver.is_current(session_id, revision)
                || session.execution.revision != Some(revision)
            {
                bail!("object actions expired");
            }
            let file = session
                .execution
                .candidates
                .get(candidate_id.as_str())
                .context("file expired")?
                .clone();
            if !file.presentation.primary_action || file.context().is_none() {
                bail!("choose an object result");
            }
            session
                .resolution
                .object_action_epoch
                .fetch_add(1, Ordering::AcqRel);
            (
                file,
                session.actions.clone(),
                session.facts(),
                session.usage.clone(),
                self.resolver.execution_epoch.load(Ordering::Acquire),
                self.resolver.action_catalog_version.load(Ordering::Acquire),
                session.resolution.inputs_epoch,
            )
        };
        let usage_of = |command: &str, kind: &str| {
            usage
                .get(&(command.into(), kind.into()))
                .copied()
                .unwrap_or(UsageStats::default())
        };
        let engine = ActionSearch::new(
            &registry,
            &usage_of,
            &facts,
            super::session_runtime::now(),
            crate::features::context_semantic,
        );
        let mut actions = if file.presentation.kind == CandidateKind::File {
            search::result_actions(std::slice::from_ref(&file), &engine)
        } else {
            engine
                .search(
                    &crate::core::search::query::Query::parse(""),
                    std::slice::from_ref(file.context().expect("validated object context")),
                    &[],
                )
                .candidates
        };
        // Opening an object's actions preserves its default operation first.
        // Other operations retain the shared ranking, including Usage. A new
        // object's default must not become Forget merely due to an ID tie.
        actions
            .sort_by_key(|action| action.presentation.command_id != file.presentation.command_id);
        {
            let mut session = self.session()?;
            if session.suspended
                || !self.resolver.is_current(session_id, revision)
                || self.resolver.execution_epoch.load(Ordering::Acquire) != epoch
                || self.resolver.action_catalog_version.load(Ordering::Acquire) != catalog
                || session.resolution.inputs_epoch != inputs
            {
                bail!("object actions expired");
            }
            session.resolution.object_actions = actions;
            session.note_snapshot_inputs();
        }
        self.resolver.recompute_current(session_id);
        let state = self
            .latest_state(session_id)
            .context("object actions expired")?;
        if state.revision != revision
            || state
                .result_actions
                .iter()
                .any(|a| a.execution_context_id != file.presentation.execution_context_id)
        {
            bail!("object actions changed");
        }
        Ok(state.result_actions)
    }
}
