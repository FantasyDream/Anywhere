//! Resolve operations only for the object the user opened, not every loaded row.
use super::Application;
#[cfg(test)]
use crate::core::ranking::score::UsageStats;
use crate::{
    core::{
        action::descriptor::ActionRegistry,
        ranking::usage::{lookup_usage, UsageSnapshot},
        search::{candidate::Candidate, engine::ActionSearch},
        session::facts::SessionFacts,
    },
    features::filesystem::search,
};
use anyhow::{bail, Context, Result};
use anywhere_protocol::*;
use std::sync::{atomic::Ordering, Arc};

/// Issued at selection time, before workers can reorder two user requests.
pub struct ObjectActionSelection {
    pub(super) session_id: SessionId,
    pub(super) revision: Revision,
    order: u64,
}

pub(super) struct ObjectActionTicket {
    pub session_id: SessionId,
    pub revision: Revision,
    pub registry: Arc<ActionRegistry>,
    pub facts: SessionFacts,
    pub usage: Arc<UsageSnapshot>,
    pub epoch: u64,
    pub catalog: u64,
    pub inputs: u64,
    pub generation: Arc<std::sync::atomic::AtomicU64>,
    pub expected: u64,
}
struct ObjectActionRequest {
    ticket: ObjectActionTicket,
    file: Candidate,
}

pub(super) fn resolve_offers(
    registry: &ActionRegistry,
    facts: &SessionFacts,
    usage: &UsageSnapshot,
    resolve: impl FnOnce(&ActionSearch<'_>) -> Vec<Candidate>,
) -> Vec<Candidate> {
    let usage_of = |command: &str, kind: &str| lookup_usage(usage, command, kind);
    let engine = ActionSearch::new(
        registry,
        &usage_of,
        facts,
        super::session_runtime::now(),
        crate::features::context_semantic,
    );
    resolve(&engine)
}

impl Application {
    pub fn select_action_object(
        &self,
        session_id: SessionId,
        revision: Revision,
    ) -> ObjectActionSelection {
        ObjectActionSelection {
            session_id,
            revision,
            order: self.object_selection_order.fetch_add(1, Ordering::Relaxed) + 1,
        }
    }

    pub(super) fn object_action_ticket_locked(
        &self,
        session: &mut crate::application::session_model::LauncherSession,
        selection: ObjectActionSelection,
    ) -> Result<ObjectActionTicket> {
        let ObjectActionSelection {
            session_id,
            revision,
            order,
        } = selection;
        anyhow::ensure!(
            !session.suspended
                && self.resolver.is_current(session_id, revision)
                && session.execution.revision == Some(revision)
                && order > session.resolution.last_object_selection,
            "object actions expired"
        );
        session.resolution.last_object_selection = order;
        let generation = session.resolution.object_action_epoch.clone();
        let expected = generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(ObjectActionTicket {
            session_id,
            revision,
            registry: session.actions.clone(),
            facts: session.facts(),
            usage: session.usage.clone(),
            epoch: self.resolver.session.execution_epoch(),
            catalog: self.resolver.catalog.version(),
            inputs: session.resolution.inputs_epoch,
            generation,
            expected,
        })
    }

    pub(super) fn publish_object_actions(
        &self,
        ticket: &ObjectActionTicket,
        actions: Vec<Candidate>,
    ) -> Result<LauncherState> {
        {
            let mut session = self.session()?;
            anyhow::ensure!(
                !session.suspended
                    && self.resolver.is_current(ticket.session_id, ticket.revision)
                    && self.resolver.session.execution_epoch() == ticket.epoch
                    && self.resolver.catalog.version() == ticket.catalog
                    && session.resolution.inputs_epoch == ticket.inputs
                    && ticket.generation.load(Ordering::Acquire) == ticket.expected,
                "object actions expired"
            );
            session.resolution.object_actions = actions;
            session.note_snapshot_inputs();
        }
        let state = self
            .resolver
            .recompute_current(ticket.session_id)
            .context("object actions expired")?;
        anyhow::ensure!(state.revision == ticket.revision, "object actions expired");
        Ok(state)
    }

    pub fn file_actions(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<Vec<CandidatePresentation>> {
        let selection = self.select_action_object(session_id, revision);
        let state = self.file_actions_snapshot(selection, candidate_id)?;
        Ok(Arc::unwrap_or_clone(state.result_actions))
    }

    /// The native view consumes the exact snapshot validated for the selected
    /// object, even if another query publishes before the reply is delivered.
    pub fn file_actions_snapshot(
        &self,
        selection: ObjectActionSelection,
        candidate_id: CandidateId,
    ) -> Result<LauncherState> {
        let request = self.prepare_object_actions(selection, candidate_id)?;
        self.resolve_object_actions(request)
    }

    fn prepare_object_actions(
        &self,
        selection: ObjectActionSelection,
        candidate_id: CandidateId,
    ) -> Result<ObjectActionRequest> {
        let mut session = self.session()?;
        let ticket = self.object_action_ticket_locked(&mut session, selection)?;
        let file = session
            .execution_candidate(candidate_id.as_str())
            .context("file expired")?
            .clone();
        if !file.presentation.primary_action || file.context().is_none() {
            bail!("choose an object result");
        }
        Ok(ObjectActionRequest { ticket, file })
    }

    fn resolve_object_actions(&self, request: ObjectActionRequest) -> Result<LauncherState> {
        let ObjectActionRequest { ticket, file } = request;
        let object_context = file.context().expect("validated object context");
        let (expanded, _) = ticket
            .registry
            .for_contexts(std::slice::from_ref(object_context));
        let mut actions = resolve_offers(&expanded, &ticket.facts, &ticket.usage, |engine| {
            if file.presentation.kind == CandidateKind::File {
                search::result_actions(std::slice::from_ref(&file), engine)
            } else {
                engine
                    .search(
                        &crate::core::search::query::Query::parse(""),
                        std::slice::from_ref(object_context),
                        &[],
                    )
                    .candidates
            }
        });
        // Opening an object's actions preserves its default operation first.
        // Other operations retain the shared ranking, including Usage. A new
        // object's default must not become Forget merely due to an ID tie.
        actions
            .sort_by_key(|action| action.presentation.command_id != file.presentation.command_id);
        let state = self.publish_object_actions(&ticket, actions)?;
        if state.revision != ticket.revision
            || state
                .result_actions
                .iter()
                .any(|a| a.execution_context_id != file.presentation.execution_context_id)
        {
            bail!("object actions changed");
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{builtin_registry, source_catalog_tests::QuietPlatform},
        core::{
            context::{ContextValue, ResolvedContext},
            ranking::usage::MemoryUsage,
            search::query::Query,
        },
    };

    #[test]
    fn latest_object_request_wins_before_either_result_is_published() {
        let registry = builtin_registry();
        let facts = SessionFacts::default();
        let usage = |_: &str, _: &str| UsageStats::default();
        let engine = ActionSearch::new(
            &registry,
            &usage,
            &facts,
            0,
            crate::features::context_semantic,
        );
        let objects = ["/fixtures/alpha", "/fixtures/beta"].map(|path| {
            let context = ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.into()),
                ContextSource::UserInput,
                "test",
            );
            engine
                .search(&Query::parse(""), &[context], &[])
                .candidates
                .into_iter()
                .find(|candidate| {
                    candidate.presentation.command_id.as_deref() == Some("core.directory.open")
                })
                .unwrap()
        });
        let app = Application::new(
            Arc::new(QuietPlatform),
            registry,
            Box::<MemoryUsage>::default(),
            vec![],
        );
        app.resolver.mark_current(1, 0);
        {
            let mut session = app.session().unwrap();
            session.state.session_id = 1;
            session.actions = app.resolver.base_registry();
            session.execution.revision = Some(0);
            session.resolution.committed_revision = Some(0);
            for object in &objects {
                session
                    .execution
                    .candidates
                    .insert(object.presentation.id.clone().into(), object.clone());
            }
        }
        let queued_first = app.select_action_object(1, 0);
        let old = app
            .prepare_object_actions(
                app.select_action_object(1, 0),
                objects[0].presentation.id.clone(),
            )
            .unwrap();
        let current = app
            .prepare_object_actions(
                app.select_action_object(1, 0),
                objects[1].presentation.id.clone(),
            )
            .unwrap();
        // The worker for the earliest selection starts last.
        assert!(app
            .prepare_object_actions(queued_first, objects[0].presentation.id.clone())
            .is_err());
        // Hold the newer resolution open while the old one reaches publication.
        // The old request must not change inputs_epoch and invalidate the new one.
        assert!(app.resolve_object_actions(old).is_err());
        let resolved = app.resolve_object_actions(current).unwrap();
        let later = app.resolver.recompute_current(1).unwrap();
        assert!(later.snapshot_seq > resolved.snapshot_seq);
        assert!(Arc::ptr_eq(&resolved.result_actions, &later.result_actions));
        let actions = resolved.result_actions;
        assert!(!actions.is_empty());
        assert!(actions
            .iter()
            .all(|action| action.execution_context_id
                == objects[1].presentation.execution_context_id));
    }
}
