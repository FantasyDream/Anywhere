//! Resolve one selected history object through the existing graph and registry.
use super::Application;
use crate::{
    core::{
        action::descriptor::ActionExecutor,
        context::{
            engine::{ContextGraph, GraphStep},
            ContextValue, ResolvedContext,
        },
        ranking::score::UsageStats,
        search::{
            candidate::{CandidateGuard, CandidateTarget},
            engine::ActionSearch,
            query::Query,
        },
    },
    features::application_history::{model::Target, service::HistoryService},
};
use anyhow::{ensure, Context, Result};
use anywhere_protocol::*;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct HistoryObjectGuard {
    history: Arc<HistoryService>,
    item: String,
    path: PathBuf,
    generation: Arc<AtomicU64>,
    expected: u64,
}
impl std::fmt::Debug for HistoryObjectGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoryObjectGuard")
    }
}
impl CandidateGuard for HistoryObjectGuard {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.generation.load(Ordering::Acquire) == self.expected,
            "object actions expired"
        );
        self.history
            .with_validated_item(&self.item, |item, cancelled| {
                let seed =
                    local_context(&item.entry.target)?.context("not a local history object")?;
                ensure!(
                    seed.value.path() == Some(self.path.as_path()) && !cancelled(),
                    "history object changed"
                );
                ensure!(
                    self.generation.load(Ordering::Acquire) == self.expected,
                    "object actions expired"
                );
                Ok(())
            })
    }
}

fn local_context(target: &Target) -> Result<Option<ResolvedContext>> {
    let Some(path) = target.local_path() else {
        return Ok(None);
    };
    let directory = matches!(
        target,
        Target::Directory { .. } | Target::ObsidianVault { .. }
    );
    ensure!(
        if directory {
            path.is_dir()
        } else {
            path.is_file()
        },
        "history target unavailable"
    );
    let path = path.canonicalize()?;
    // A workspace stays a file. Its parents and its first root are not a project.
    let (kind, value) = if directory {
        (DIRECTORY, ContextValue::Directory(path))
    } else {
        (FILE, ContextValue::File(path))
    };
    Ok(Some(ResolvedContext::new(
        kind,
        value,
        ContextSource::UserInput,
        "application.history.object",
    )))
}

impl Application {
    pub async fn history_object_actions(
        &self,
        history: Arc<HistoryService>,
        session_id: SessionId,
        revision: Revision,
        item_id: String,
    ) -> Result<Vec<CandidatePresentation>> {
        let (registry, facts, usage, epoch, catalog, inputs, generation, expected) = {
            let session = self.session()?;
            ensure!(
                !session.suspended
                    && self.resolver.is_current(session_id, revision)
                    && session.execution.revision == Some(revision),
                "object actions expired"
            );
            let generation = session.resolution.object_action_epoch.clone();
            let expected = generation.fetch_add(1, Ordering::AcqRel) + 1;
            (
                session.actions.clone(),
                session.facts(),
                session.usage.clone(),
                self.resolver.execution_epoch.load(Ordering::Acquire),
                self.resolver.action_catalog_version.load(Ordering::Acquire),
                session.resolution.inputs_epoch,
                generation,
                expected,
            )
        };
        let service = history.clone();
        let selected = item_id.clone();
        let (seed, cancelled) = tokio::task::spawn_blocking(move || {
            service.with_validated_item(&selected, |item, cancelled| {
                Ok((local_context(&item.entry.target)?, cancelled.clone()))
            })
        })
        .await??;
        let started = Instant::now();
        let mut ticket = self.resolver.execution(session_id, revision);
        let policy = ticket.guard.take();
        let guard_epoch = generation.clone();
        ticket.guard = Some(Arc::new(move || {
            !cancelled()
                && guard_epoch.load(Ordering::Acquire) == expected
                && started.elapsed() < Duration::from_secs(2)
                && policy.as_ref().is_none_or(|guard| guard())
        }));
        let mut run = ContextGraph::start(seed.clone().into_iter().collect(), &ticket);
        let providers = self
            .resolver
            .providers
            .lock()
            .map_err(|_| anyhow::anyhow!("providers unavailable"))?
            .providers()
            .to_vec();
        let contexts = loop {
            ensure!(
                ticket.is_current(),
                "object resolution expired or exceeded its budget"
            );
            match run.step(&providers) {
                GraphStep::Done { contexts, .. } => break contexts,
                GraphStep::Pending => {
                    let outcome = run
                        .await_pending(Some(started + Duration::from_secs(2)), None, started)
                        .await
                        .context("object resolution exceeded its budget")?;
                    run.settle(outcome);
                }
            }
        };
        let mut actions = {
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
            engine.search(&Query::parse(""), &contexts, &[]).candidates
        };
        // This first-party workflow does not add new Web grants or legacy DSL access.
        actions.retain(|candidate| {
            matches!(&candidate.target, CandidateTarget::Action { action, .. }
            if matches!(action.executor, ActionExecutor::Native(_)) && !action.executor.is_menu())
        });
        // Copy Path is this object's primary menu command. Other commands retain
        // the shared ordering; the page opens this menu from its row or right-click.
        actions.sort_by_key(|candidate| {
            !matches!(
                candidate.presentation.command_id.as_deref(),
                Some("core.directory.copy" | "core.file.copy")
            )
        });
        let prefix = format!("history:{item_id}:{expected}:");
        if let Some(seed) = seed {
            let guard = Arc::new(HistoryObjectGuard {
                history,
                item: item_id,
                path: seed
                    .value
                    .path()
                    .context("local object has no path")?
                    .into(),
                generation: generation.clone(),
                expected,
            });
            let validate = guard.clone();
            tokio::task::spawn_blocking(move || validate.validate()).await??;
            for action in &mut actions {
                action.presentation.id = format!("{prefix}{}", action.presentation.id);
                action.guard = Some(guard.clone());
            }
        }
        {
            let mut session = self.session()?;
            ensure!(
                !session.suspended
                    && ticket.is_current()
                    && self.resolver.execution_epoch.load(Ordering::Acquire) == epoch
                    && self.resolver.action_catalog_version.load(Ordering::Acquire) == catalog
                    && session.resolution.inputs_epoch == inputs,
                "object actions expired"
            );
            session.resolution.object_actions = actions;
            session.note_snapshot_inputs();
        }
        self.resolver.recompute_current(session_id);
        let state = self
            .latest_state(session_id)
            .context("object actions expired")?;
        ensure!(
            state.revision == revision
                && state
                    .result_actions
                    .iter()
                    .all(|action| action.id.starts_with(&prefix)),
            "object actions changed"
        );
        Ok(state.result_actions)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{
        application::source_catalog_tests::QuietPlatform,
        core::{
            action::{
                availability::AvailabilityRule,
                bundle::ActionSpec,
                descriptor::ActionRegistry,
                handler::{ActionHandler, ActionHandlerRef, ExecutionOutput},
            },
            ranking::usage::MemoryUsage,
            search::candidate::CandidateGuard,
            session::facts::SessionFacts,
        },
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    struct Noop;
    impl ActionHandler for Noop {
        fn execute(
            &self,
            _: &ResolvedContext,
            _: &dyn anywhere_platform::Platform,
        ) -> Result<ExecutionOutput> {
            Ok(ExecutionOutput::dismiss())
        }
    }
    #[derive(Debug)]
    struct Guard {
        current: Arc<AtomicBool>,
        calls: AtomicUsize,
    }
    impl CandidateGuard for Guard {
        fn validate(&self) -> Result<()> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            ensure!(
                self.current.load(Ordering::Acquire),
                "object revoked during planning"
            );
            Ok(())
        }
    }
    struct PlanAndRevoke {
        current: Arc<AtomicBool>,
        directory: PathBuf,
    }
    impl ActionHandler for PlanAndRevoke {
        fn execute(
            &self,
            _: &ResolvedContext,
            _: &dyn anywhere_platform::Platform,
        ) -> Result<ExecutionOutput> {
            self.current.store(false, Ordering::Release);
            Ok(ExecutionOutput::process(
                anywhere_platform::process::ProcessRequest {
                    title: "must not start".into(),
                    program: "/bin/echo".into(),
                    arguments: vec![],
                    directory: self.directory.clone(),
                    environment: Default::default(),
                },
            ))
        }
    }
    #[tokio::test]
    async fn object_revocation_during_planning_prevents_process_spawn() {
        let temp = tempfile::tempdir().unwrap();
        let current = Arc::new(AtomicBool::new(true));
        let guard = Arc::new(Guard {
            current: current.clone(),
            calls: AtomicUsize::new(0),
        });
        let mut definition = ActionSpec {
            id: "test.plan",
            title: "Plan",
            localized_titles: &[],
            accepts: DIRECTORY,
            handler: &Noop,
            availability: AvailabilityRule::Enabled,
            base_priority: 0,
            icon: "folder",
            aliases: &[],
            keywords: &[],
            primary_action: false,
        }
        .descriptor("folder");
        definition.executor = ActionExecutor::Native(ActionHandlerRef::owned(PlanAndRevoke {
            current,
            directory: temp.path().into(),
        }));
        let mut registry = ActionRegistry::default();
        registry.register(definition).unwrap();
        let facts = SessionFacts::default();
        let usage = |_: &str, _: &str| UsageStats::default();
        let engine = ActionSearch::new(
            &registry,
            &usage,
            &facts,
            0,
            crate::features::context_semantic,
        );
        let context = ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(temp.path().into()),
            ContextSource::UserInput,
            "test",
        );
        let mut candidate = engine
            .search(&Query::parse(""), &[context], &[])
            .candidates
            .remove(0);
        candidate.guard = Some(guard.clone());
        let id = candidate.presentation.id.clone();
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
            session.execution.revision = Some(0);
            session.execution.action_catalog_version = Some(1);
            session
                .execution
                .candidates
                .insert(id.clone().into(), candidate);
        }
        let result = app.execute_use_case(1, 0, id).await.unwrap();
        assert_eq!(result.status, ActionStatus::Failed);
        assert!(app.execution_sessions().is_empty());
        assert_eq!(guard.calls.load(Ordering::Relaxed), 2);
    }
}
